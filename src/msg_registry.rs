use std::any::{TypeId, Any};
use std::collections::HashMap;
use bincode::deserialize;
use errors::*;

/// A cluster-wide, globally unique MsgId. MsgIds for a given type must be the same on every node.
///
/// MsgIds may never change or be re-used.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MsgId(u32);

/// A Registry stores the mappings of TypeId to MsgId and MsgId to TypeId for the given type
///
/// Note that since TypeId of a given type can change across builds, it is not possible to use it
/// globally in clusters. Therefore, at program startup the TypeId for a given type is mapped to an
/// unchanging MsgId.
#[derive(Debug, Clone)]
pub struct Registry {
    msg_ids: HashMap<TypeId, MsgId>,
    type_ids: HashMap<MsgId, TypeId>
}

impl Registry {
    /// Create a new registry
    pub fn new() -> Registry {
        Registry {
            msg_ids: HashMap::new(),
            type_ids: HashMap::new()
        }
    }

    /// Create a new entry in each table
    ///
    /// Panics if the key already exists, as this can lead to terrible, very hard to track bugs.
    pub fn add_mapping(&mut self, type_id: TypeId, msg_id: MsgId) {
        match msg_ids.insert(type_id, msg_id) {
            Some(current) => panic!("TypeId {} already present in registry. \
                                     Tried to insert MsgId {}, but {} already existed!",
                                     type_id, msg_id, current),
            None => ()
        }
        match type_ids.insert(msg_id, type_id) {
            Some(current) => panic!("MsgId {} already present in registry. \
                                     Tried to insert TypeId {}, but {} already existed!",
                                     type_id, msg_id, current),
            None => ()
        }
    }
}

/// For a given registry, add the appropriate Type/MsgId mappings
///
/// let mut registry = Registry::new();
/// register!(registry, {
///    String => 1,
///    u64 => 2
/// });
///
#[macro_export]
macro_rules! register {
    ($registry:ident, {
        $( $ty:ty => $msg_id:expr),+
    }) => {
        $(
            $registry.add_mapping(TypeId::of::<$ty>(), MsgId($msg_id));
        )+
    }
}

/// Register all known message types with rabble
///
/// This must be done exactly once at the start of the application
///
/// init_msg_registry!(
///     String => 1,
///     u64 => 2
/// );
#[macro_export]
macro_rules! init_msg_registry {
    $( $ty:ty => $msg_id:expr),+ => {

        lazy_static! {
            static ref RABBLE_MSG_REGISTRY: Registry = {
                let mut registry = Registry::new();
                // Register user defined types
                register!(registry, {
                    $(
                        ($ty => $msg_id)
                    )+
                }

                // Register rabble types
                rabble_msgs.register(&mut registry);
            }
        }

    }
}


/// Decode a message into its concrete type and execute the corresponding clause where the type
/// matches.
///
/// If the msg doesn't match a known type or there is an error, the default `_` clause is entered.
///
///  * `msg` is a `Msg` type as received in an `Envelope` and is the type to be decoded.
///  * `concrete` is an identifier that gets set to the decoded type on success so it can be
///     accessed in the specific clause that matched. The name of the identifier can be anything.
///  * `error` is an identifier that gets set to the error if `msg` cannot be decoded so that it
///     can be accessed in the default clause.
///
///  Note that the type to match comes before the `=>` just like in a `match` statement. Also note
///  that in order to prevent macro parser ambiguity, the last type match arm does *not* have a
///  comma. This is required, or a compiler error will occur unfortunately.
///
/// decode!(msg, concrete, error {
///    String => {
///        println!("Match succeeded: concrete value = {:?}", concrete);
///    },
///    u64 => {
///        println!("Match succeeded: concrete value = {:?}", concrete);
///    }
///    _ => {
///        println!("Match failed: error = {:?}", error);
///    }
/// });
#[macro_export]
macro_rules! decode {
    ($msg:ident, $concrete:ident, $error:ident {
        $( $ty:ty => $type_handler:block),+
        $ident => $err_handler:block
    }) => {
        match RABBLE_MSG_REGISTRY.type_ids.get($msg.id) {
            Some(type_id) => {
                if false {}
            $(
                else if TypeId::of::<$ty>() == type_id {
                    let result = match $msg.data {
                        Local(value) => {
                            value.downcast::<$ty>()
                                .map(|data| *data)
                                .map_err(|_| ErrorKind::DowncastFailed($msg.id).into())
                        },
                        Remote(buf) => {
                            deserialize::<$ty>(buf).map_err(|_| {
                                ErrorKind::DeserializerError($msg.id).into()
                            })
                        }
                    };
                    match result {
                        Ok($concrete) => $type_handler,
                        Err($error) => $err_handler
                    }
                }
            )+
                else {
                    let $error = ErrorKind::UnexpectedMsg(msg.id).into();
                    $err_handler
                }
            }
            None => {
                let $error = ErrorKind::UnregisteredMsg(msg.id).into();
                $err_handler
            }
        }
    }
}

