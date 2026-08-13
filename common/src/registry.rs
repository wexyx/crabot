use crate::factory::TypedBeanFactory;
use crate::inventory::FactoryRegistration;
use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::Error;
use once_cell::sync::Lazy;

static BEAN_REGISTRY: Lazy<AnyBeanRegistry> = Lazy::new(AnyBeanRegistry::new);

pub fn register() {
    for r in inventory::iter::<FactoryRegistration> {
        (r.register_fn)(&BEAN_REGISTRY);
    }
}

pub fn create<I, O>(bean: &str, input: I) -> Result<O, Error>
where
    I: 'static,
    O: 'static,
{
    let factory = BEAN_REGISTRY
        .get_factory::<I, O>()
        .ok_or(anyhow::Error::msg(format!(
            "factory not found: {}_{}",
            type_name::<I>(),
            type_name::<O>()
        )))?;

    let demo = factory
        .create(bean, input)
        .ok_or(anyhow::Error::msg(format!(
            "bean: {} not found: {}_{}",
            bean,
            type_name::<I>(),
            type_name::<O>()
        )))?;
    Ok(demo)
}

pub fn factory<I, O>() -> Result<Arc<TypedBeanFactory<I, O>>, Error>
where
    I: 'static,
    O: 'static,
{
    let factory = BEAN_REGISTRY
        .get_factory::<I, O>()
        .ok_or(anyhow::Error::msg(format!(
            "factory not found: {}_{}",
            type_name::<I>(),
            type_name::<O>()
        )))?;
    Ok(factory.clone())
}

pub struct AnyBeanRegistry {
    factories: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl AnyBeanRegistry {
    pub fn new() -> Self {
        Self {
            factories: RwLock::new(HashMap::new()),
        }
    }

    pub fn init_factory<I: 'static, O: 'static>(&self) -> Option<Arc<TypedBeanFactory<I, O>>> {
        let factory = self
            .factories
            .write()
            .unwrap()
            .entry(TypeId::of::<(I, O)>())
            .or_insert_with(|| Arc::new(TypedBeanFactory::<I, O>::new()))
            .clone();

        let result = factory.downcast::<TypedBeanFactory<I, O>>();
        match result {
            Ok(r) => {
                return Some(r);
            }
            Err(_) => {
                return None;
            }
        }
    }

    pub fn get_factory<I: 'static, O: 'static>(&self) -> Option<Arc<TypedBeanFactory<I, O>>> {
        let map = self.factories.read().unwrap();
        if let Some(factory) = map.get(&TypeId::of::<(I, O)>()) {
            let result = factory.clone().downcast::<TypedBeanFactory<I, O>>();
            match result {
                Ok(r) => {
                    return Some(r);
                }
                Err(_) => {
                    return None;
                }
            }
        }

        None
    }
}
