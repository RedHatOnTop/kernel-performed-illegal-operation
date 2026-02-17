//! WASI P2 Component Bridge
//!
//! Registers WASI Preview 2 interfaces (`wasi:clocks`, `wasi:random`,
//! `wasi:cli`, etc.) as component model imports so that components
//! can use standard WASI interfaces through the ComponentLinker.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::linker::{ComponentHostFn, ComponentLinker, HostExport, InterfaceInstance};
use super::{ComponentError, ComponentType, ComponentValue};

/// Register all WASI P2 interfaces on the given linker.
pub fn register_wasi_p2(linker: &mut ComponentLinker) -> Result<(), ComponentError> {
    register_clocks_monotonic(linker)?;
    register_clocks_wall(linker)?;
    register_random(linker)?;
    register_cli_environment(linker)?;
    register_cli_stdout(linker)?;
    Ok(())
}

/// Register `wasi:clocks/monotonic-clock@0.2.0`.
fn register_clocks_monotonic(linker: &mut ComponentLinker) -> Result<(), ComponentError> {
    let iface = InterfaceInstance {
        name: String::from("wasi:clocks/monotonic-clock@0.2.0"),
        exports: alloc::vec![
            HostExport {
                name: String::from("now"),
                params: alloc::vec![],
                results: alloc::vec![ComponentType::U64],
                func: wasi_clock_now,
            },
            HostExport {
                name: String::from("resolution"),
                params: alloc::vec![],
                results: alloc::vec![ComponentType::U64],
                func: wasi_clock_resolution,
            },
        ],
    };
    linker.define_instance(iface)
}

fn wasi_clock_now(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    // MVP: return a simulated monotonic timestamp.
    // In a real kernel, this would use rdtsc or HPET.
    Ok(alloc::vec![ComponentValue::U64(1_000_000)])
}

fn wasi_clock_resolution(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    Ok(alloc::vec![ComponentValue::U64(1_000)]) // 1μs resolution
}

/// Register `wasi:clocks/wall-clock@0.2.0`.
fn register_clocks_wall(linker: &mut ComponentLinker) -> Result<(), ComponentError> {
    let iface = InterfaceInstance {
        name: String::from("wasi:clocks/wall-clock@0.2.0"),
        exports: alloc::vec![HostExport {
            name: String::from("now"),
            params: alloc::vec![],
            results: alloc::vec![ComponentType::Record(alloc::vec![
                (String::from("seconds"), ComponentType::U64),
                (String::from("nanoseconds"), ComponentType::U32),
            ])],
            func: wasi_wall_clock_now,
        }],
    };
    linker.define_instance(iface)
}

fn wasi_wall_clock_now(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    Ok(alloc::vec![ComponentValue::Record(alloc::vec![
        (String::from("seconds"), ComponentValue::U64(1700000000)),
        (String::from("nanoseconds"), ComponentValue::U32(0)),
    ])])
}

/// Register `wasi:random/random@0.2.0`.
fn register_random(linker: &mut ComponentLinker) -> Result<(), ComponentError> {
    let iface = InterfaceInstance {
        name: String::from("wasi:random/random@0.2.0"),
        exports: alloc::vec![
            HostExport {
                name: String::from("get-random-bytes"),
                params: alloc::vec![ComponentType::U64],
                results: alloc::vec![ComponentType::List(Box::new(ComponentType::U8))],
                func: wasi_random_bytes,
            },
            HostExport {
                name: String::from("get-random-u64"),
                params: alloc::vec![],
                results: alloc::vec![ComponentType::U64],
                func: wasi_random_u64,
            },
        ],
    };
    linker.define_instance(iface)
}

fn wasi_random_bytes(args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    let len = match &args[0] {
        ComponentValue::U64(v) => *v as usize,
        _ => return Err(ComponentError::TypeMismatch(String::from("expected u64"))),
    };
    // MVP: deterministic pseudo-random bytes
    let mut rng = crate::wasi2::random::RandomGenerator::new();
    let bytes = rng.get_random_bytes(len);
    let list: Vec<ComponentValue> = bytes.into_iter().map(ComponentValue::U8).collect();
    Ok(alloc::vec![ComponentValue::List(list)])
}

fn wasi_random_u64(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    let mut rng = crate::wasi2::random::RandomGenerator::new();
    Ok(alloc::vec![ComponentValue::U64(rng.get_random_u64())])
}

/// Register `wasi:cli/environment@0.2.0`.
fn register_cli_environment(linker: &mut ComponentLinker) -> Result<(), ComponentError> {
    let iface = InterfaceInstance {
        name: String::from("wasi:cli/environment@0.2.0"),
        exports: alloc::vec![
            HostExport {
                name: String::from("get-environment"),
                params: alloc::vec![],
                results: alloc::vec![ComponentType::List(Box::new(ComponentType::Record(
                    alloc::vec![
                        (String::from("key"), ComponentType::String),
                        (String::from("value"), ComponentType::String),
                    ],
                )))],
                func: wasi_get_environment,
            },
            HostExport {
                name: String::from("get-arguments"),
                params: alloc::vec![],
                results: alloc::vec![ComponentType::List(Box::new(ComponentType::String))],
                func: wasi_get_arguments,
            },
        ],
    };
    linker.define_instance(iface)
}

fn wasi_get_environment(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    // Return empty environment in sandboxed context
    Ok(alloc::vec![ComponentValue::List(alloc::vec![])])
}

fn wasi_get_arguments(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    // Return empty arguments in sandboxed context
    Ok(alloc::vec![ComponentValue::List(alloc::vec![])])
}

/// Register `wasi:cli/stdout@0.2.0`.
fn register_cli_stdout(linker: &mut ComponentLinker) -> Result<(), ComponentError> {
    let iface = InterfaceInstance {
        name: String::from("wasi:cli/stdout@0.2.0"),
        exports: alloc::vec![HostExport {
            name: String::from("get-stdout"),
            params: alloc::vec![],
            results: alloc::vec![ComponentType::U32], // resource handle
            func: wasi_get_stdout,
        }],
    };
    linker.define_instance(iface)
}

fn wasi_get_stdout(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
    // Return a handle representing stdout (handle 1)
    Ok(alloc::vec![ComponentValue::U32(1)])
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_wasi_p2_all() {
        let mut linker = ComponentLinker::new();
        register_wasi_p2(&mut linker).unwrap();

        assert!(linker.has_instance("wasi:clocks/monotonic-clock@0.2.0"));
        assert!(linker.has_instance("wasi:clocks/wall-clock@0.2.0"));
        assert!(linker.has_instance("wasi:random/random@0.2.0"));
        assert!(linker.has_instance("wasi:cli/environment@0.2.0"));
        assert!(linker.has_instance("wasi:cli/stdout@0.2.0"));
    }

    #[test]
    fn test_wasi_clock_now() {
        let result = wasi_clock_now(&[]).unwrap();
        assert_eq!(result.len(), 1);
        if let ComponentValue::U64(v) = &result[0] {
            assert!(*v > 0);
        } else {
            panic!("expected U64");
        }
    }

    #[test]
    fn test_wasi_clock_resolution() {
        let result = wasi_clock_resolution(&[]).unwrap();
        assert_eq!(result.len(), 1);
        if let ComponentValue::U64(v) = &result[0] {
            assert_eq!(*v, 1_000);
        } else {
            panic!("expected U64");
        }
    }

    #[test]
    fn test_wasi_wall_clock_now() {
        let result = wasi_wall_clock_now(&[]).unwrap();
        assert_eq!(result.len(), 1);
        if let ComponentValue::Record(fields) = &result[0] {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "seconds");
            assert_eq!(fields[1].0, "nanoseconds");
        } else {
            panic!("expected Record");
        }
    }

    #[test]
    fn test_wasi_random_bytes() {
        let result = wasi_random_bytes(&[ComponentValue::U64(16)]).unwrap();
        assert_eq!(result.len(), 1);
        if let ComponentValue::List(bytes) = &result[0] {
            assert_eq!(bytes.len(), 16);
        } else {
            panic!("expected List");
        }
    }

    #[test]
    fn test_wasi_random_u64() {
        let result = wasi_random_u64(&[]).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ComponentValue::U64(_)));
    }

    #[test]
    fn test_wasi_get_environment() {
        let result = wasi_get_environment(&[]).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ComponentValue::List(_)));
    }

    #[test]
    fn test_wasi_get_arguments() {
        let result = wasi_get_arguments(&[]).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ComponentValue::List(_)));
    }

    #[test]
    fn test_wasi_get_stdout() {
        let result = wasi_get_stdout(&[]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ComponentValue::U32(1));
    }

    #[test]
    fn test_wasi_p2_linker_resolve_clock() {
        let mut linker = ComponentLinker::new();
        register_wasi_p2(&mut linker).unwrap();

        let export = linker
            .resolve("wasi:clocks/monotonic-clock@0.2.0", "now")
            .unwrap();
        assert_eq!(export.name, "now");
        let result = (export.func)(&[]).unwrap();
        assert!(matches!(result[0], ComponentValue::U64(_)));
    }

    #[test]
    fn test_wasi_p2_instantiate_component_with_wasi() {
        use super::super::linker::ComponentExport;

        let mut linker = ComponentLinker::new();
        register_wasi_p2(&mut linker).unwrap();

        fn my_app(_args: &[ComponentValue]) -> Result<Vec<ComponentValue>, ComponentError> {
            Ok(alloc::vec![ComponentValue::U32(0)]) // exit code 0
        }

        let exports = alloc::vec![ComponentExport {
            name: String::from("run"),
            params: alloc::vec![],
            results: alloc::vec![ComponentType::U32],
            func: Some(my_app),
        }];

        let instance = linker
            .instantiate(
                &[
                    "wasi:clocks/monotonic-clock@0.2.0",
                    "wasi:random/random@0.2.0",
                ],
                exports,
            )
            .unwrap();

        // Call the component's export
        let result = instance.call("run", &[]).unwrap();
        assert_eq!(result, alloc::vec![ComponentValue::U32(0)]);

        // Call a WASI import from within the component
        let clock = instance
            .call_import("wasi:clocks/monotonic-clock@0.2.0", "now", &[])
            .unwrap();
        assert!(matches!(clock[0], ComponentValue::U64(_)));
    }
}
