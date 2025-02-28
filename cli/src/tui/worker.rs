use super::monotonic_counter::MonotonicCounter;
use database::{Backend, Database, Filter, Mailbox, MailboxInfo, Message, State};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, channel};
use std::thread;
use tokio::runtime::Handle;

pub enum Request {
    InitialLoad {
        filter: Filter,
        initial_mailbox: Option<Mailbox>,
    },
    LoadMessages(Filter),
    LoadMailboxes(Filter),
    ChangeMessageStates {
        filter: Filter,
        new_state: State,
        refresh: bool,
    },
    DeleteMessages {
        filter: Filter,
        refresh: bool,
    },
}

pub enum Response {
    InitialLoad {
        mailboxes: Vec<MailboxInfo>,
        messages: Vec<Message>,
        initial_mailbox: Option<Mailbox>,
    },
    LoadMessages(Vec<Message>),
    LoadMailboxes(Vec<MailboxInfo>),
    Mutated {
        refresh: bool,
    },
}

pub type Sender = mpsc::Sender<Request>;
pub type Receiver = mpsc::Receiver<Response>;

// Spawn a worker for asynchronously interacting with the database
// It receives requests from a channel, runs the corresponding database query asynchronously,
// and when the response is ready, sends it on another channel
pub fn spawn<B: Backend + Send + Sync + 'static>(
    db: Arc<Database<B>>,
    pending_requests: Arc<AtomicUsize>,
) -> (Sender, Receiver) {
    let (tx_req, rx_req) = channel::<Request>();
    let (tx_res, rx_res) = channel::<Response>();

    let handle = Handle::current();
    let message_counter = MonotonicCounter::new();
    let mailbox_counter = MonotonicCounter::new();
    thread::spawn(move || {
        loop {
            let Ok(req) = rx_req.recv() else { break };
            let tx_res = tx_res.clone();
            let db = Arc::clone(&db);
            let message_counter = message_counter.clone();
            let mailbox_counter = mailbox_counter.clone();
            let pending_requests = Arc::clone(&pending_requests);
            handle.spawn(async move {
                match req {
                    Request::InitialLoad {
                        filter,
                        initial_mailbox,
                    } => {
                        // Load the mailboxes and messages in parallel
                        pending_requests.fetch_add(1, Ordering::Relaxed);
                        let mailbox_db = Arc::clone(&db);
                        let mailbox_filter = filter.clone();
                        let mailboxes =
                            tokio::spawn(
                                async move { mailbox_db.load_mailboxes(mailbox_filter).await },
                            );

                        let messages_filter = filter.with_mailbox_option(initial_mailbox.clone());
                        let messages =
                            tokio::spawn(async move { db.load_messages(messages_filter).await });

                        let mailboxes = mailboxes.await.unwrap().unwrap();
                        let messages = messages.await.unwrap().unwrap();
                        pending_requests.fetch_sub(1, Ordering::Relaxed);
                        tx_res
                            .send(Response::InitialLoad {
                                mailboxes,
                                messages,
                                initial_mailbox,
                            })
                            .unwrap();
                    }
                    Request::LoadMessages(filter) => {
                        pending_requests.fetch_add(1, Ordering::Relaxed);
                        let req_id = message_counter.next();
                        let messages = db.load_messages(filter).await.unwrap();
                        pending_requests.fetch_sub(1, Ordering::Relaxed);
                        // Only use these messages if there aren't any fresher load requests in progress
                        if message_counter.last() == req_id {
                            tx_res.send(Response::LoadMessages(messages)).unwrap();
                        }
                    }
                    Request::LoadMailboxes(filter) => {
                        pending_requests.fetch_add(1, Ordering::Relaxed);
                        let req_id = mailbox_counter.next();
                        let mailboxes = db.load_mailboxes(filter).await.unwrap();
                        pending_requests.fetch_sub(1, Ordering::Relaxed);
                        // Only use these mailboxes if there aren't any fresher load requests in progress
                        if mailbox_counter.last() == req_id {
                            tx_res.send(Response::LoadMailboxes(mailboxes)).unwrap();
                        }
                    }
                    Request::ChangeMessageStates {
                        filter,
                        new_state,
                        refresh,
                    } => {
                        pending_requests.fetch_add(1, Ordering::Relaxed);
                        db.change_state(filter, new_state).await.unwrap();
                        pending_requests.fetch_sub(1, Ordering::Relaxed);
                        tx_res.send(Response::Mutated { refresh }).unwrap();
                    }
                    Request::DeleteMessages { filter, refresh } => {
                        pending_requests.fetch_add(1, Ordering::Relaxed);
                        db.delete_messages(filter).await.unwrap();
                        pending_requests.fetch_sub(1, Ordering::Relaxed);
                        tx_res.send(Response::Mutated { refresh }).unwrap();
                    }
                }
            });
        }
    });

    (tx_req, rx_res)
}
