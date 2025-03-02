use automerge::{
    transaction::Transactable, AutoCommit, Change, ObjType, ReadDoc,
};
use automerge::{ObjId, Value, ROOT};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

struct NetworkMessage {
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
}

impl ChatUser {
    fn new(user_id: &str) -> Result<Self, automerge::AutomergeError> {
        let doc = AutoCommit::new();

        Ok(ChatUser {
            id: user_id.to_string(),
            doc,
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
        let message_obj = self.doc.put_object(automerge::ROOT, &msg_id, ObjType::Map)?;
        self.doc.put(&message_obj, "user_id", self.id.clone())?;
        self.doc.put(&message_obj, "content", content.to_string())?;
        self.doc.put(&message_obj, "timestamp", timestamp)?;

        let change = self.doc.get_last_local_change().unwrap();

        Ok(NetworkMessage {
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

        for entry in self.doc.map_range(ROOT, ..) {
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
        
        messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
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
        user.receive_message(msg)?;
    }
    Ok(())
}

fn main() -> Result<(), automerge::AutomergeError> {
    let mut user1 = ChatUser::new("user1")?;
    let mut user2 = ChatUser::new("user2")?;

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
    let mut user3 = ChatUser::new("user3")?;
    let get_changes: Vec<Change> = user1.doc.get_changes(&[]).into_iter().cloned().collect();

    let initial_sync = NetworkMessage {
        changes: get_changes,
    };
    user3.receive_message(&initial_sync)?;

    // User 3 sends a message
    let msg4 = user3.add_message("Hey, can I join?")?;
    broadcast_message(&msg4, &mut [&mut user1, &mut user2])?;


    user1.print_messages();
    user2.print_messages();
    user3.print_messages();

    Ok(())
}
