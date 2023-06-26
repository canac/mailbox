use super::request_counter::RequestCounter;
use database::{Database, Message, MessageFilter, State};
use std::sync::mpsc::{self, channel};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Handle;

pub enum Request {
    LoadMessages(MessageFilter),
    LoadMailboxes(MessageFilter),
    ChangeMessageStates {
        filter: MessageFilter,
        new_state: State,
    },
    DeleteMessages(MessageFilter),
}

pub enum Response {
    LoadMessages(Vec<Message>),
    LoadMailboxes(Vec<(String, usize)>),
    ChangeMessageStates,
    DeleteMessages,
}

pub type Sender = mpsc::Sender<Request>;
pub type Receiver = mpsc::Receiver<Response>;

// Spawn an worker for asynchronously interacting with the database
// It receives requests from a channel, runs the corresponding database query asynchronously,
// and when the response is ready, sends it on another channel
pub fn spawn(db: Arc<Database>) -> (Sender, Receiver) {
    let (tx_req, rx_req) = channel::<Request>();
    let (tx_res, rx_res) = channel::<Response>();

    let handle = Handle::current();
    let message_req_counter = RequestCounter::new();
    let mailbox_req_counter = RequestCounter::new();
    thread::spawn(move || loop {
        let Ok(req) = rx_req.recv() else { break };
        let tx_res = tx_res.clone();
        let db: Arc<Database> = db.clone();
        let message_req_counter = message_req_counter.clone();
        let mailbox_req_counter = mailbox_req_counter.clone();
        handle.spawn(async move {
            match req {
                Request::LoadMessages(filter) => {
                    let req_id = message_req_counter.next();
                    let messages = db.load_messages(filter).await.unwrap();
                    // Only use these messages if there aren't any fresher load requests in progress
                    if message_req_counter.is_latest(&req_id) {
                        tx_res.send(Response::LoadMessages(messages)).unwrap();
                    }
                }
                Request::LoadMailboxes(filter) => {
                    let req_id = mailbox_req_counter.next();
                    let mailboxes = db.load_mailboxes(filter).await.unwrap();
                    // Only use these mailboxes if there aren't any fresher load requests in progress
                    if mailbox_req_counter.is_latest(&req_id) {
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
