use super::monotonic_counter::MonotonicCounter;
use database::{Backend, Database, Filter, MailboxInfo, Message, State};
use std::sync::mpsc::{self, channel};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Handle;

pub enum Request {
    LoadMessages(Filter),
    LoadMailboxes(Filter),
    ChangeMessageStates { filter: Filter, new_state: State },
    DeleteMessages(Filter),
}

pub enum Response {
    LoadMessages(Vec<Message>),
    LoadMailboxes(Vec<MailboxInfo>),
    ChangeMessageStates,
    DeleteMessages,
}

pub type Sender = mpsc::Sender<Request>;
pub type Receiver = mpsc::Receiver<Response>;

// Spawn a worker for asynchronously interacting with the database
// It receives requests from a channel, runs the corresponding database query asynchronously,
// and when the response is ready, sends it on another channel
pub fn spawn<B: Backend + Send + Sync + 'static>(db: Arc<Database<B>>) -> (Sender, Receiver) {
    let (tx_req, rx_req) = channel::<Request>();
    let (tx_res, rx_res) = channel::<Response>();

    let handle = Handle::current();
    let message_counter = MonotonicCounter::new();
    let mailbox_counter = MonotonicCounter::new();
    thread::spawn(move || loop {
        let Ok(req) = rx_req.recv() else { break };
        let tx_res = tx_res.clone();
        let db = db.clone();
        let message_counter = message_counter.clone();
        let mailbox_counter = mailbox_counter.clone();
        handle.spawn(async move {
            match req {
                Request::LoadMessages(filter) => {
                    let req_id = message_counter.next();
                    let messages = db.load_messages(filter).await.unwrap();
                    // Only use these messages if there aren't any fresher load requests in progress
                    if message_counter.last() == req_id {
                        tx_res.send(Response::LoadMessages(messages)).unwrap();
                    }
                }
                Request::LoadMailboxes(filter) => {
                    let req_id = mailbox_counter.next();
                    let mailboxes = db.load_mailboxes(filter).await.unwrap();
                    // Only use these mailboxes if there aren't any fresher load requests in progress
                    if mailbox_counter.last() == req_id {
                        tx_res.send(Response::LoadMailboxes(mailboxes)).unwrap();
                    }
                }
                Request::ChangeMessageStates { filter, new_state } => {
                    db.change_state(filter, new_state).await.unwrap();
                    tx_res.send(Response::ChangeMessageStates).unwrap();
                }
                Request::DeleteMessages(filter) => {
                    db.delete_messages(filter).await.unwrap();
                    tx_res.send(Response::DeleteMessages).unwrap();
                }
            }
        });
    });

    (tx_req, rx_res)
}
