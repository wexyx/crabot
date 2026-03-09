use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::constructor::Constructor;

pub struct TypedBeanFactory<I, O> {
    pub constructors: RwLock<HashMap<String, Arc<dyn Constructor<I, O>>>>,
}

impl<I: 'static, O: 'static> TypedBeanFactory<I, O> {
    pub fn new() -> Self {
        Self {
            constructors: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(
        &self,
        name: &str,
        ctor: Arc<dyn Constructor<I, O>>,
    ) {
        println!("TypedBeanFactory register bean: {}", name);
        self.constructors
            .write()
            .unwrap()
            .insert(name.to_string(), ctor);
    }

    pub fn create(&self, name: &str, input: I) -> Option<O> {
        self.constructors
            .read()
            .unwrap()
            .get(name)
            .map(|c| {
                c.create(input)
            })
    }

    pub fn constructors(&self) -> HashMap<String, Arc<dyn Constructor<I, O>>> {
        let constructors = self.constructors
            .read()
            .unwrap();

        let mut results = HashMap::new();
        for (key, constructor) in constructors.iter() {
            results.insert(key.clone(), constructor.clone());
        }

        results
    }
}
