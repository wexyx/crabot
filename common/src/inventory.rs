use crate::registry::AnyBeanRegistry;

pub struct FactoryRegistration {
    pub register_fn: fn(&AnyBeanRegistry),
}

inventory::collect!(FactoryRegistration);
