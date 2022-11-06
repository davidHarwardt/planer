use std::sync::{Arc, Weak, Mutex};

use serde::{Deserialize, Serialize};
use uuid::Uuid;


#[derive(Debug, Deserialize, Serialize)]
pub struct UuidRef<T> {
    #[serde(skip, default = "std::sync::Weak::new")]
    item: Weak<T>,
    uuid: Uuid,
}

impl<T> Clone for UuidRef<T> {
    fn clone(&self) -> Self {
        Self { item: Weak::clone(&self.item), uuid: self.uuid }
    }
}

impl<T: AsUuid> UuidRef<T> {
    pub fn new(v: &Arc<T>) -> Self {
        Self { item: Arc::downgrade(v), uuid: v.as_uuid() }
    }

    pub fn null() -> Self {
        Self { item: Weak::new(), uuid: Uuid::from_u128(0) }
    }

    pub fn revalidate(&mut self, data: &[Arc<T>]) {
        if let Some(data) = data.iter().find(|v| v.as_uuid() == self.uuid) {
            self.item = Arc::downgrade(data);
        }
    }

    pub fn uuid(&self) -> Uuid { self.uuid }


    pub fn get(&self) -> Option<Arc<T>> { Weak::upgrade(&self.item) }
}


pub trait AsUuid {
    fn as_uuid(&self) -> Uuid;
}

impl<T: AsUuid> AsUuid for Mutex<T> {
    fn as_uuid(&self) -> Uuid {
        self.lock().unwrap().as_uuid()
    }
}


