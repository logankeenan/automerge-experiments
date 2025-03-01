use automerge::{
    iter::MapRangeItem, transaction::Transactable, AutoCommit, Change, ObjType, ReadDoc,
};
use automerge::{ActorId, Value};
use std::time::{SystemTime, UNIX_EPOCH};

struct NetworkMessage {
    from_user: String,
    changes: Vec<Change>,
}

#[derive(Debug)]
struct ChatMessage {
    user_id: String,
    content: String,
    timestamp: u64,
}

struct ChatUser {
    id: String,
    doc: AutoCommit,
    messages_id: automerge::ObjId,
}

impl ChatUser {
    fn new(user_id: &str, doc: AutoCommit, messages_id: automerge::ObjId) -> Result<Self, automerge::AutomergeError> {
        // let mut doc = AutoCommit::new();
// 
        // let messages_id = doc.put_object(automerge::ROOT, "messages", ObjType::Map)?;

        Ok(ChatUser {
            id: user_id.to_string(),
            doc,
            messages_id,
        })
    }

    fn add_message(&mut self, content: &str) -> Result<NetworkMessage, automerge::AutomergeError> {
        // Generate timestamp for message ID and message data
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let msg_id = format!("msg_{}", timestamp);

        // Create message as a Map entry
        let message_obj = self
            .doc
            .put_object(&self.messages_id, &msg_id, ObjType::Map)?;
        self.doc.put(&message_obj, "user_id", self.id.clone())?;
        self.doc.put(&message_obj, "content", content.to_string())?;
        self.doc.put(&message_obj, "timestamp", timestamp)?;

        let change = self.doc.get_last_local_change().unwrap();

        Ok(NetworkMessage {
            from_user: self.id.clone(),
            changes: vec![change.clone()],
        })
    }

    fn receive_message(
        &mut self,
        network_msg: &NetworkMessage,
    ) -> Result<(), automerge::AutomergeError> {
        for change in &network_msg.changes {
            self.doc.apply_changes(vec![change.clone()])?;
        }
        Ok(())
    }

    fn get_messages(&self) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        for entry in self.doc.map_range(&self.messages_id, ..) {
            
            
            let user_id = match self.doc.get(&entry.id, "user_id") {
                Ok(Some((Value::Scalar(user_id), _))) => user_id.to_str().unwrap().to_string(),
                _ => continue,
            };
            let content = match self.doc.get(&entry.id, "content") {
                Ok(Some((Value::Scalar(content), _))) => content.to_str().unwrap().to_string(),
                _ => continue,
            };

            let timestamp = match self.doc.get(&entry.id, "timestamp") {
                Ok(Some((Value::Scalar(timestamp), _))) => timestamp.to_u64().unwrap(),
                _ => continue,
            };

            messages.push(ChatMessage {
                user_id,
                content,
                timestamp,
            });
        }

        messages
    }

    fn print_messages(&self) {
        println!("\nChat Messages (from {}'s perspective):", self.id);
        println!("-------------");

        let messages = self.get_messages();
        for msg in messages {
            println!("[{}] User {}: {}", msg.timestamp, msg.user_id, msg.content);
        }

        println!("-------------\n");
    }
}

fn broadcast_message(
    msg: &NetworkMessage,
    users: &mut [&mut ChatUser],
) -> Result<(), automerge::AutomergeError> {
    for user in users {
        // if user.id != msg.from_user {
        user.receive_message(msg)?;
        // }
    }
    Ok(())
}

fn main() -> Result<(), automerge::AutomergeError> {
    let mut base_doc = AutoCommit::new();
    let messages_id = base_doc.put_object(automerge::ROOT, "messages", ObjType::Map)?;

    let mut user1 = ChatUser::new("user1", base_doc.fork(), messages_id.clone())?;
    let mut user2 = ChatUser::new("user2", base_doc.fork(), messages_id.clone())?;

    // User 1 sends a message
    let msg1 = user1.add_message("Hello, anyone there?")?;
    broadcast_message(&msg1, &mut [&mut user2])?;

    // Small delay to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(100));

    // User 2 responds
    let msg2 = user2.add_message("Hi! Yes, I'm here!")?;
    broadcast_message(&msg2, &mut [&mut user1])?;

    std::thread::sleep(std::time::Duration::from_millis(100));

    // User 1 responds again
    let msg3 = user1.add_message("Great to see you!")?;
    broadcast_message(&msg3, &mut [&mut user2])?;

    // Create user3 and sync with user1's state
    let mut user3 = ChatUser::new("user3", base_doc.fork(), messages_id.clone())?;
    let get_changes : Vec<Change> = user1.doc.get_changes(&[]).into_iter().cloned().collect();
    

    let initial_sync = NetworkMessage {
        from_user: user1.id.clone(),
        changes: get_changes,
    };
    user3.receive_message(&initial_sync)?;
    
    // User 3 sends a message
    let msg4 = user3.add_message("Hey, can I join?")?;
    broadcast_message(&msg4, &mut [&mut user1, &mut user2])?;
    
    // Print final state from all users' perspectives
    user1.print_messages();
    user2.print_messages();
    user3.print_messages();

    Ok(())
}
