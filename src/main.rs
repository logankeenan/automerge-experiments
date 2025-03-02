use automerge::sync::SyncDoc;
use automerge::{sync, Value, ROOT};
use automerge::{transaction::Transactable, AutoCommit, Change, ObjType, ReadDoc};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

struct NetworkMessage {
    changes: Vec<Change>,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    id: Uuid,
    user_id: String,
    content: String,
    timestamp: u64,
}

impl ChatMessage {
    fn new(user_id: &str, content: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        ChatMessage {
            id: Uuid::new_v4(),
            user_id: user_id.to_string(),
            content: content.to_string(),
            timestamp,
        }
    }
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

    fn add_message(
        &mut self,
        chat_message: ChatMessage,
    ) -> Result<NetworkMessage, automerge::AutomergeError> {
        

        // Create message as a Map entry
        let message_obj = self
            .doc
            .put_object(automerge::ROOT, &chat_message.id.to_string(), ObjType::Map)?;
        self.doc
            .put(&message_obj, "id", chat_message.id.to_string())?;
        self.doc
            .put(&message_obj, "user_id", chat_message.user_id)?;
        self.doc
            .put(&message_obj, "content", chat_message.content.to_string())?;
        self.doc
            .put(&message_obj, "timestamp", chat_message.timestamp)?;

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
            
            let id = match self.doc.get(&entry.id, "id") {
                Ok(Some((Value::Scalar(content), _))) => content.to_str().unwrap().to_string(),
                _ => continue,
            };

            let timestamp = match self.doc.get(&entry.id, "timestamp") {
                Ok(Some((Value::Scalar(timestamp), _))) => timestamp.to_u64().unwrap(),
                _ => continue,
            };

            messages.push(ChatMessage {
                id: Uuid::parse_str(&id).unwrap(),
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
    let mut user1_message1 = ChatMessage::new("user1", "Hello, anyone there?");
    let msg1 = user1.add_message(user1_message1.clone())?;
    broadcast_message(&msg1, &mut [&mut user2])?;

    // Small delay to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(100));

    // User 2 responds
    let msg2 = user2.add_message(ChatMessage::new("user2", "Hi! Yes, I'm here!"))?;
    broadcast_message(&msg2, &mut [&mut user1])?;

    std::thread::sleep(std::time::Duration::from_millis(100));

    // User 1 responds again
    let msg3 = user1.add_message(ChatMessage::new("user1", "Great to see you!"))?;
    broadcast_message(&msg3, &mut [&mut user2])?;

    // Create user3 and sync with user1's state

    let mut user3 = ChatUser::new("user3")?;

    let mut user1_state = sync::State::new();
    // Generate the initial message to send to user2, unwrap for brevity
    let message1to2 = user1
        .doc
        .sync()
        .generate_sync_message(&mut user1_state)
        .unwrap();

    let mut user3_state = sync::State::new();
    user3
        .doc
        .sync()
        .receive_sync_message(&mut user3_state, message1to2)?;

    loop {
        let two_to_one = user3.doc.sync().generate_sync_message(&mut user3_state);
        if let Some(message) = two_to_one.as_ref() {
            user1
                .doc
                .sync()
                .receive_sync_message(&mut user1_state, message.clone())?;
        }
        let one_to_two = user1.doc.sync().generate_sync_message(&mut user1_state);
        if let Some(message) = one_to_two.as_ref() {
            user3
                .doc
                .sync()
                .receive_sync_message(&mut user3_state, message.clone())?;
        }
        if two_to_one.is_none() && one_to_two.is_none() {
            break;
        }
    }

    // User 3 sends a message
    let msg4 = user3.add_message(ChatMessage::new("user3", "Hey, can I join?"))?;
    broadcast_message(&msg4, &mut [&mut user1, &mut user2])?;
    
    // user 1 modified the first message
    user1_message1.content = "Hello, anyone there??? [Edit]".to_string();
    let msg1 = user1.add_message(user1_message1)?;
    broadcast_message(&msg1, &mut [&mut user2, &mut user3])?;

    user1.print_messages();
    user2.print_messages();
    user3.print_messages();

    Ok(())
}
