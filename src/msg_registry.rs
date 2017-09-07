use std::any::{TypeId, Any};
use std::collections::HashMap;
use bincode::deserialize;

/// A cluster-wide, globally unique MsgId. MsgIds for a given type must be the same on every node.
///
/// MsgIds may never change or be re-used.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MsgId(u32);

/// An error when a msg fails to deserialize given a buffer and it's MsgId
#[derive(Debug, Clone)]
pub struct DeserializeError(pub MsgId);

/// A function that takes a bunch of bytes, deserializes it to specific type, then converts the type
/// to a Box<Any>. If the deserialization fails, a DeserializeError is returned with the given
/// MsgType for the attempted deserialization.
type Deserializer = Box< Fn(&[u8]) -> Result<Box<Any>, DeserializeError> >;

/// A Registry stores the mappings of TypeId to MsgId and MsgId to a deserializer for the given type
///
/// Note that since TypeId of a given type can change across builds, it is not possible to use it
/// globally in clusters. Therefore, at program startup the TypeId for a given type is mapped to an
/// unchanging MsgId.
#[derive(Debug, Clone)]
pub struct Registry {
    msg_ids: HashMap<TypeId, MsgId>,
    deserializers: HashMap<MsgId, Deserializer>
}

impl Registry {
    /// Create a new registry
    pub fn new() -> Registry {
        Registry {
            msg_ids: HashMap::new(),
            deserializers: HashMap::new()
        }
    }

    /// Create a new entry in each table
    ///
    /// Panics if the key already exists, as this can lead to terrible, very hard to track bugs.
    pub fn add_mapping(&mut self, type_id: TypeId, msg_id: MsgId, deserializer: Deserializer) {
        match msg_ids.insert(type_id, msg_id) {
            Some(current) => panic!("TypeId {} already present in registry. \
                                     Tried to insert MsgId {}, but {} already existed!",
                                     type_id, msg_id, current),
            None => ()
        }
        match deserializers.insert(msg_id, deserializer) {
            Some(current) => panic!("MsgId {} deserializer already present in registry!", msg_id),
            None => ()
        }
    }

    pub fn get_msg_id(&self, type_id: TypeId) -> Option<MsgId> {
    }
}

/// For a given registry, add the appropriate Type/MsgId and MsgId/Deserializer mappings
///
/// let mut registry = Registry::new();
/// register!(registry, {
///    String => 1,
///    u64 => 2
/// });
///
#[macro_export]
macro_rules! register {
    ($registry:ident {
        $( $ty:ty => $msg_id:expr),+
    }) => {
        $(
            let deserializer = Box::new(|buf| {
                deserialize::<$ty>(buf).map_ok(|concrete| Box<concrete> as Box<Any>)
                                       .map_err(|_| DeserializeError($msg_id))
            });
            registry.add_mapping(TypeId::of::<$ty>(), $msg_id, deserializer);
        )+
    }
}

