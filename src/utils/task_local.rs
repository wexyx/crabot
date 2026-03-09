use std::{cell::RefCell, collections::HashMap, hash::Hash, sync::{Arc, RwLock}};
use std::any::Any;
use tokio::task_local;

task_local! {
    pub static TASK_LOCAL: RefCell<Option<Arc<TaskLocal>>>;
}

pub fn get_task_local() -> Arc<TaskLocal> {
    let data = TASK_LOCAL.try_with(|v| {
        let b = v;
        if b.borrow().is_none() {
            *b.borrow_mut() = Some(Arc::new(TaskLocal::new()))
        }

        b.borrow().as_ref().unwrap().clone()
    });

    let r= match data {
        Ok(result) => {
            result
        },
        Err(_err) => {
            Arc::new(TaskLocal::new())
        }
    };

    r
}

pub fn set<T: 'static + Clone + Send + Sync>(key: &str, value: T) {
    let local = get_task_local();
    local.set(key, value);
}

pub fn get<T: 'static + Clone + Send + Sync>(key: &str) -> Option<T> {
    let local = get_task_local();
    local.get(key)
}

/// 当前不是标准的task_local，目前所有的异步任务共享同一个TaskLocal实例，即：子任务写入的数据也能被父任务读取。
/// 后续可以考虑改成Copy或者链表形式，解决父子数据复制，以及子任务数据隔离问题
pub struct TaskLocal {
    data: TaskLocalData<String, Box<dyn Any + Send + Sync>>
}

impl TaskLocal {
    fn new() -> Self {
        Self { 
            data: TaskLocalData::new()
        }
    }

    pub fn set<T: 'static + Clone + Send + Sync>(&self, key: &str, value: T) {
        self.data.set(key.to_string(), Arc::new(Box::new(value)));
    }

    pub fn get<T: 'static + Clone + Send + Sync>(&self, key: &str) -> Option<T> {
        let key_string = key.to_string();
        self.data.get(&key_string)?.downcast_ref::<T>().cloned()
    }
}

struct TaskLocalData<K: Eq + Hash, V> {
    data: RwLock<HashMap<K, Arc<V>>>
}

impl <K: Eq + Hash, V> TaskLocalData<K, V> {
    pub fn new() -> Self {
        return Self {
            data: Default::default()
        };
    }

    pub fn set(&self, k: K, v: Arc<V>) {
        let mut data =self.data.write().unwrap();
        data.insert(k, v);
    }

    pub fn get(&self, k: &K) -> Option<Arc<V>> {
        let data =self.data.read().unwrap();
        let v = data.get(k);
        v.cloned()
    }

    pub fn remove(&self, k: &K) {
        let mut data =self.data.write().unwrap();
        data.remove(k);
    }

    pub fn clear(&self) {
        let mut data =self.data.write().unwrap();
        data.clear();
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_task_local_data() {
        let data = TaskLocal::new();
        data.set("1", 2222 as i32);
        data.set("2", "3333");
        data.set("3", 0.1 as f32);

        let v1 = data.get::<i32>("1");
        assert!(v1.is_some());
        assert!(v1.unwrap_or_default() == 2222);

        let v2 = data.get::<&str>("2");
        assert!(v2.unwrap() == "3333");

        let v3 = data.get::<f32>("3");
        assert!(v3.unwrap() == 0.1);

        let v4 = data.get::<i32>("4");
        assert!(v4.is_none());

        data.data.remove(&"1".to_string());
        let v1 = data.get::<i32>("1");
        assert!(v1.is_none());

        let v2 = data.get::<&str>("2");
        assert!(v2.unwrap() == "3333");

        data.data.clear();
        let v2 = data.get::<&str>("2");
        assert!(v2.is_none());
    }
}