//! ErrorRecoveryCoordinator with crossbeam message-passing architecture
//!
//! This module provides proper crossbeam message-passing for error recovery operations,
//! following the same patterns as Hot and Warm tier coordinators.

use std::any::TypeId;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use crossbeam_channel::{bounded, Sender};
use dashmap::DashMap;

use crate::cache::traits::types_and_enums::CacheOperationError;
use crate::cache::traits::{CacheKey, CacheValue};


/// Error recovery message for worker routing
#[allow(dead_code)] // Error recovery - message system used in distributed error recovery coordination
#[derive(Debug)]
pub enum ErrorRecoveryMessage<K: CacheKey, V: CacheValue> {
    ConfigurationReset {
        response: Sender<Result<(), CacheOperationError>>,
    },
    SystemRestart {
        response: Sender<Result<(), CacheOperationError>>,
    },
    CircuitBreakerReset {
        tier: u8,
        response: Sender<Result<(), CacheOperationError>>,
    },
    HealthCheck {
        response: Sender<super::core::SystemHealthReport>,
    },
    GetErrorStats {
        response: Sender<super::statistics::ErrorStatistics>,
    },
    Shutdown,
    _PhantomData(std::marker::PhantomData<(K, V)>),
}

/// Trait for type-erased error recovery operations
#[allow(dead_code)] // Error recovery - operations trait used in polymorphic error recovery systems
trait ErrorRecoveryOperations: std::any::Any + Send + Sync {
    fn shutdown(&self);
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Handle for communicating with error recovery instance
#[allow(dead_code)] // Error recovery - handle used in coordinated error recovery communication
struct ErrorRecoveryHandle<K: CacheKey, V: CacheValue> {
    sender: Sender<ErrorRecoveryMessage<K, V>>,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K: CacheKey, V: CacheValue> Clone for ErrorRecoveryHandle<K, V> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K: CacheKey, V: CacheValue> ErrorRecoveryOperations for ErrorRecoveryHandle<K, V> {
    fn shutdown(&self) {
        let _ = self.sender.send(ErrorRecoveryMessage::Shutdown);
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Global error recovery coordinator for type-safe operations
#[allow(dead_code)] // Error recovery - coordinator used in global error recovery management
pub struct ErrorRecoveryCoordinator {
    /// Storage for different K,V type combinations using DashMap
    recovery_workers: DashMap<(TypeId, TypeId), Box<dyn ErrorRecoveryOperations>>,
    /// Instance counter for load balancing
    
    instance_selector: AtomicUsize,
}

#[allow(dead_code)] // Error recovery - global coordinator instance used in system-wide error recovery
static COORDINATOR: std::sync::OnceLock<ErrorRecoveryCoordinator> = std::sync::OnceLock::new();

impl ErrorRecoveryCoordinator {
    /// Initialize the global coordinator
    #[allow(dead_code)] // Error recovery - initialize used in error recovery system startup
    pub fn initialize() -> Result<(), CacheOperationError> {
        COORDINATOR.get_or_init(|| ErrorRecoveryCoordinator {
            recovery_workers: DashMap::new(),
            instance_selector: AtomicUsize::new(0),
        });
        Ok(())
    }

    /// Get the global coordinator instance
    #[allow(dead_code)] // Error recovery - get used in error recovery system access
    fn get() -> Result<&'static ErrorRecoveryCoordinator, CacheOperationError> {
        COORDINATOR.get().ok_or_else(|| {
            CacheOperationError::invalid_state("ErrorRecoveryCoordinator not initialized")
        })
    }

    /// Get or create an error recovery worker for the given K,V types
    #[allow(dead_code)] // Error recovery - get_or_create_worker used in dynamic worker management
    fn get_or_create_worker<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
        &self,
    ) -> Result<ErrorRecoveryHandle<K, V>, CacheOperationError> {
        let type_key = (TypeId::of::<K>(), TypeId::of::<V>());
        
        // Try to get existing worker
        if let Some(handle_ops) = self.recovery_workers.get(&type_key) {
            if let Some(handle) = handle_ops.as_any().downcast_ref::<ErrorRecoveryHandle<K, V>>() {
                return Ok(handle.clone());
            }
        }
        
        // Create new worker if doesn't exist
        let (sender, receiver) = bounded::<ErrorRecoveryMessage<K, V>>(1024);
        
        // Spawn background task to handle error recovery operations - worker OWNS the data
        std::thread::spawn(move || {
            // Create worker-owned error recovery system
            let mut error_recovery = super::core::ErrorRecoverySystem::new();
            
            while let Ok(request) = receiver.recv() {
                match request {
                    ErrorRecoveryMessage::ConfigurationReset { response } => {
                        let result = Self::execute_configuration_reset_generic::<K, V>(&error_recovery);
                        let _ = response.send(result);
                    }
                    ErrorRecoveryMessage::SystemRestart { response } => {
                        let result = Self::execute_system_restart_generic::<K, V>(&mut error_recovery);
                        let _ = response.send(result);
                    }
                    ErrorRecoveryMessage::CircuitBreakerReset { tier, response } => {
                        error_recovery.circuit_breaker.reset_tier(tier);
                        let _ = response.send(Ok(()));
                    }
                    ErrorRecoveryMessage::HealthCheck { response } => {
                        let health_report = error_recovery.perform_health_check();
                        let _ = response.send(health_report);
                    }
                    ErrorRecoveryMessage::GetErrorStats { response } => {
                        let stats = error_recovery.error_stats.clone();
                        let _ = response.send(stats);
                    }
                    ErrorRecoveryMessage::Shutdown => break,
                    ErrorRecoveryMessage::_PhantomData(_) => {} // Phantom data, no-op
                }
            }
        });
        
        let handle = ErrorRecoveryHandle { 
            sender,
            _phantom: std::marker::PhantomData,
        };
        self.recovery_workers.insert(type_key, Box::new(handle.clone()));
        Ok(handle)
    }

    /// Execute configuration reset with proper generic types
    #[allow(dead_code)] // Error recovery - execute_configuration_reset_generic used in system configuration reset
    fn execute_configuration_reset_generic<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
        error_recovery: &super::core::ErrorRecoverySystem<K, V>,
    ) -> Result<(), CacheOperationError> {
        use crate::cache::config::CacheConfig;

        
        // 1. Apply default CacheConfig settings
        let default_config = CacheConfig::default();
        
        // 2. Reinitialize all tiers with new configuration using existing init functions
        // Hot tier reinitialization with PROPER GENERIC TYPES
        let hot_tier_config = default_config.hot_tier.clone();
        
        // Reinitialize tiers with proper generic types K, V
        if let Err(e) = crate::cache::tier::hot::init_simd_hot_tier::<K, V>(hot_tier_config) {
            return Err(CacheOperationError::io_failed(&format!("Hot tier init failed during config reset: {}", e)));
        }
        
        // Warm tier reinitialization with PROPER GENERIC TYPES
        let warm_tier_config = default_config.warm_tier.clone();
        if let Err(e) = crate::cache::tier::warm::init_warm_tier::<K, V>(warm_tier_config) {
            return Err(CacheOperationError::io_failed(&format!("Warm tier init failed during config reset: {}", e)));
        }
        
        // Cold tier reinitialization with PROPER GENERIC TYPES
        if let Err(e) = crate::cache::tier::cold::init_cold_tier::<K, V>(default_config.cold_tier.storage_path.as_str()) {
            return Err(CacheOperationError::io_failed(&format!("Cold tier init failed during config reset: {}", e)));
        }
        
        // 3. Reinitialize coherence protocol with PROPER GENERIC TYPES
        let _coherence_sender = crate::cache::coherence::protocol::global_api::init_coherence_system::<K, V>().map_err(|_| CacheOperationError::InternalError)?;
        
        // 4. Update circuit breaker state to closed after successful reset
        error_recovery.circuit_breaker.reset();
        
        Ok(())
    }

    /// Execute system restart with proper crossbeam coordination
    #[allow(dead_code)] // Error recovery - execute_system_restart_generic used in system restart operations
    fn execute_system_restart_generic<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(
        error_recovery: &mut super::core::ErrorRecoverySystem<K, V>,
    ) -> Result<(), CacheOperationError> {
        // 1. Gracefully restart all tier coordinators using their crossbeam messaging
        
        // Hot tier restart - use existing crossbeam messaging
        crate::cache::tier::hot::thread_local::initialize_hot_tier_system()?;
        
        // Warm tier restart - use existing crossbeam messaging
        crate::cache::tier::warm::global_api::init_warm_tier_system()?;
        
        // Cold tier restart - already uses crossbeam messaging internally
        // The ColdTierCoordinator handles its own restart through message passing
        
        // 2. Reset error recovery system state
        error_recovery.error_stats.reset_statistics();
        error_recovery.circuit_breaker.reset();
        error_recovery.recovery_strategies.reset_success_rates();
        
        // 3. Reinitialize with proper generic types
        let default_config = crate::cache::config::CacheConfig::default();
        
        let hot_tier_config = default_config.hot_tier.clone();
        
        // Reinitialize with proper generic types
        crate::cache::tier::hot::init_simd_hot_tier::<K, V>(hot_tier_config)?;
        crate::cache::tier::warm::init_warm_tier::<K, V>(default_config.warm_tier.clone())?;
        crate::cache::tier::cold::init_cold_tier::<K, V>(default_config.cold_tier.storage_path.as_str())
            .map_err(|e| CacheOperationError::io_failed(&format!("Cold tier restart failed: {}", e)))?;
        
        // 4. Reinitialize coherence protocol
        let _coherence_sender = crate::cache::coherence::protocol::global_api::init_coherence_system::<K, V>().map_err(|_| CacheOperationError::InternalError)?;
        
        Ok(())
    }

    /// Shutdown all error recovery workers
    #[allow(dead_code)] // Error recovery - shutdown_all used in system shutdown and cleanup
    fn shutdown_all(&self) {
        for entry in self.recovery_workers.iter() {
            entry.value().shutdown();
        }
        self.recovery_workers.clear();
    }
}

/// Initialize error recovery coordinator system
#[allow(dead_code)] // Error recovery - init_error_recovery_system used in system initialization
pub fn init_error_recovery_system() -> Result<(), CacheOperationError> {
    ErrorRecoveryCoordinator::initialize()
}

/// Execute configuration reset via worker-based routing
#[allow(dead_code)] // Error recovery - execute_configuration_reset used in configuration recovery operations
pub fn execute_configuration_reset<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>() -> Result<(), CacheOperationError> {
    let coordinator = ErrorRecoveryCoordinator::get()?;
    let handle = coordinator.get_or_create_worker::<K, V>()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = ErrorRecoveryMessage::ConfigurationReset {
        response: response_tx,
    };
    
    handle.sender.send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx.recv_timeout(Duration::from_millis(10000)) // 10 second timeout for reinitialization
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Execute system restart via worker-based routing
#[allow(dead_code)] // Error recovery - execute_system_restart used in system restart operations
pub fn execute_system_restart<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>() -> Result<(), CacheOperationError> {
    let coordinator = ErrorRecoveryCoordinator::get()?;
    let handle = coordinator.get_or_create_worker::<K, V>()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = ErrorRecoveryMessage::SystemRestart {
        response: response_tx,
    };
    
    handle.sender.send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx.recv_timeout(Duration::from_millis(10000)) // 10 second timeout for restart
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Reset circuit breaker for specific tier via worker-based routing
#[allow(dead_code)] // Error recovery - reset_circuit_breaker used in circuit breaker recovery operations
pub fn reset_circuit_breaker<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>(tier: u8) -> Result<(), CacheOperationError> {
    let coordinator = ErrorRecoveryCoordinator::get()?;
    let handle = coordinator.get_or_create_worker::<K, V>()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = ErrorRecoveryMessage::CircuitBreakerReset {
        tier,
        response: response_tx,
    };
    
    handle.sender.send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx.recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)?
}

/// Get system health report via worker-based routing
#[allow(dead_code)] // Error recovery - get_system_health used in system health monitoring
pub fn get_system_health<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>() -> Result<super::core::SystemHealthReport, CacheOperationError> {
    let coordinator = ErrorRecoveryCoordinator::get()?;
    let handle = coordinator.get_or_create_worker::<K, V>()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = ErrorRecoveryMessage::HealthCheck {
        response: response_tx,
    };
    
    handle.sender.send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx.recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)
}

/// Get error statistics via worker-based routing
#[allow(dead_code)] // Error recovery - get_error_statistics used in error statistics collection
pub fn get_error_statistics<K: CacheKey + Default + bincode::Encode + bincode::Decode<()> + 'static, V: CacheValue + Default + serde::Serialize + serde::de::DeserializeOwned + bincode::Encode + bincode::Decode<()> + 'static>() -> Result<super::statistics::ErrorStatistics, CacheOperationError> {
    let coordinator = ErrorRecoveryCoordinator::get()?;
    let handle = coordinator.get_or_create_worker::<K, V>()?;
    
    let (response_tx, response_rx) = bounded(1);
    let message = ErrorRecoveryMessage::GetErrorStats {
        response: response_tx,
    };
    
    handle.sender.send(message)
        .map_err(|_| CacheOperationError::resource_exhausted("Worker queue full"))?;
    response_rx.recv_timeout(Duration::from_millis(1000))
        .map_err(|_| CacheOperationError::TimeoutError)
}

/// Shutdown error recovery system
#[allow(dead_code)] // Error recovery - shutdown_error_recovery_system used in system shutdown
pub fn shutdown_error_recovery_system() -> Result<(), CacheOperationError> {
    if let Some(coordinator) = COORDINATOR.get() {
        coordinator.shutdown_all();
    }
    Ok(())
}

/// Execute configuration reset via type-erased tier coordination (works with any K,V types)
pub fn execute_configuration_reset_type_erased() -> Result<(), CacheOperationError> {
    // Use tier coordinators directly - they handle multiple K,V types via TypeId
    use crate::cache::tier::warm::global_api as warm_global;
    use crate::cache::tier::cold::ColdTierCoordinator;
    
    // Reset hot tier configuration (type-erased via thread local API)
    crate::cache::tier::hot::thread_local::clear_hot_tier_system()?;
    
    // Reset warm tier configuration (type-erased via global coordinators)  
    warm_global::clear_all_warm_tiers()?;
    
    // Reset cold tier configuration (type-erased) - use maintenance operation
    if let Ok(coordinator) = ColdTierCoordinator::get() {
        // Use type-erased maintenance that works with all stored types
        let _ = coordinator.execute_type_erased_maintenance("reset");
    }
    
    Ok(())
}

/// Execute system restart via type-erased tier coordination (works with any K,V types)
pub fn execute_system_restart_type_erased() -> Result<(), CacheOperationError> {
    // Use tier coordinators directly - they handle multiple K,V types via TypeId
    use crate::cache::tier::warm::global_api as warm_global;
    use crate::cache::tier::cold::ColdTierCoordinator;
    
    // Restart hot tier system (type-erased via init function)
    crate::cache::tier::hot::thread_local::initialize_hot_tier_system()?;
    
    // Restart warm tier system (type-erased via shutdown/init cycle)
    warm_global::shutdown_warm_tier()?;
    warm_global::init_warm_tier_system()?;
    
    // Restart cold tier system (type-erased) - use maintenance operation
    if let Ok(coordinator) = ColdTierCoordinator::get() {
        // Use type-erased maintenance that works with all stored types
        let _ = coordinator.execute_type_erased_maintenance("restart");
    }
    
    Ok(())
}