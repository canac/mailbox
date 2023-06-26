use super::request_counter::RequestCounter;
use database::{Database, Message, MessageFilter, MessageState};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Handle;

pub enum WorkerRequest {
    LoadMessages(MessageFilter),
    LoadMailboxes(MessageFilter),
    ChangeMessageStates {
        filter: MessageFilter,
        new_state: MessageState,
    },
    DeleteMessages(MessageFilter),
}

pub enum WorkerResponse {
    LoadMessages(Vec<Message>),
    LoadMailboxes(Vec<(String, usize)>),
    ChangeMessageStates,
    DeleteMessages,
}

pub type WorkerSender = Sender<WorkerRequest>;
pub type WorkerReceiver = Receiver<WorkerResponse>;

// Spawn an worker for asynchronously interacting with the database
// It receives requests from a channel, runs the corresponding database query asynchronously,
// and when the response is ready, sends it on another channel
pub fn start_worker(db: Arc<Database>) -> (WorkerSender, WorkerReceiver) {
    let (tx_req, rx_req) = channel::<WorkerRequest>();
    let (tx_res, rx_res) = channel::<WorkerResponse>();

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
                WorkerRequest::LoadMessages(filter) => {
                    let req_id = message_req_counter.next();
                    let messages = db.load_messages(filter).await.unwrap();
                    // Only use these messages if there aren't any fresher load requests in progress
                    if message_req_counter.is_latest(req_id) {
                        tx_res.send(WorkerResponse::LoadMessages(messages)).unwrap();
                    }
                }
                WorkerRequest::LoadMailboxes(filter) => {
                    let req_id = mailbox_req_counter.next();
                    let mailboxes = db.load_mailboxes(filter).await.unwrap();
                    // Only use these mailboxes if there aren't any fresher load requests in progress
                    if mailbox_req_counter.is_latest(req_id) {
                        tx_res
                            .send(WorkerResponse::LoadMailboxes(mailboxes))
                            .unwrap();
                    }
                }
                WorkerRequest::ChangeMessageStates { filter, new_state } => {
                    db.change_state(filter, new_state).await.unwrap();
                    tx_res.send(WorkerResponse::ChangeMessageStates).unwrap();
                }
                WorkerRequest::DeleteMessages(filter) => {
                    db.delete_messages(filter).await.unwrap();
                    tx_res.send(WorkerResponse::DeleteMessages).unwrap();
                }
            }
        });
    });

    (tx_req, rx_res)
}
