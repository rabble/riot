//! The process-singleton client internet runtime.
//!
//! [`RiotApplicationRuntime`] owns the one iroh endpoint and one Tokio runtime
//! for an application process. It is created once through a [`RuntimeHost`];
//! calling the host again returns a clone of the existing runtime instead of
//! binding a second endpoint. Background work is scoped to per-profile
//! [`ProfileLease`]s: releasing a lease cancels only that profile's tasks and
//! drains them, and [`RiotApplicationRuntime::close`] is refused until every
//! lease has been released, after which the endpoint is shut down (bounded,
//! idempotent).
//!
//! The endpoint and the task executor are injected through [`EndpointFactory`]
//! and [`TaskSpawner`] so the lifecycle is testable with fakes and no live
//! network. [`IrohEndpointFactory`] and [`TokioTaskSpawner`] are the real
//! implementations used by native shells.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Cooperative cancellation flag shared between the runtime and a spawned task.
///
/// A lease cancels its token on release; a task observes cancellation (a real
/// task is also aborted, bounding the drain).
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// A fresh, un-cancelled token.
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Request cancellation. Idempotent.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// An owner-facing profile identifier for a network lease.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProfileId(pub String);

impl ProfileId {
    /// Build a profile id from anything string-like.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Result alias for runtime lifecycle operations.
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// A lifecycle failure from the application runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// The application runtime is already closed; no further operations run.
    Closed,
    /// Application close was refused because this many profile leases are still
    /// open. Release every lease first.
    ProfileLeasesOutstanding(usize),
    /// The lease is no longer active (already released); it holds no tasks.
    LeaseReleased,
    /// The injected endpoint factory failed to create the one endpoint.
    EndpointCreation(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Closed => write!(f, "application runtime is closed"),
            RuntimeError::ProfileLeasesOutstanding(n) => {
                write!(
                    f,
                    "application close refused: {n} profile lease(s) still open"
                )
            }
            RuntimeError::LeaseReleased => write!(f, "profile lease already released"),
            RuntimeError::EndpointCreation(e) => write!(f, "endpoint creation failed: {e}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// A future run as a runtime background task.
pub type RuntimeFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Injected seam that creates the ONE process network endpoint.
///
/// The real implementation binds an iroh endpoint; tests inject a fake that
/// records how many endpoints it was asked to create (must be exactly one).
pub trait EndpointFactory {
    /// Create the single endpoint. Called at most once per runtime.
    fn create_endpoint(&self) -> RuntimeResult<Arc<dyn NetworkEndpoint>>;
}

/// The one process network endpoint. [`shutdown`](NetworkEndpoint::shutdown) is
/// bounded and idempotent.
pub trait NetworkEndpoint: Send + Sync {
    /// Close the endpoint within a bounded deadline. Safe to call once.
    fn shutdown(&self);
}

/// Injected seam that spawns background work bound to a cancellation token.
///
/// The real implementation spawns on the one Tokio runtime; tests inject a fake
/// recorder.
pub trait TaskSpawner: Send + Sync {
    /// Spawn `task`, bound to `token`. Returns a joinable handle; cancelling the
    /// token must stop the task and [`SpawnedTask::join`] must then drain it
    /// within a bounded deadline.
    fn spawn(&self, token: CancellationToken, task: RuntimeFuture) -> Box<dyn SpawnedTask>;
}

/// A joinable spawned task. [`join`](SpawnedTask::join) awaits termination
/// (drain) within a bounded deadline.
pub trait SpawnedTask: Send {
    /// Await the task's termination. Bounded.
    fn join(self: Box<Self>);
}

type LeaseId = u64;

struct LeaseState {
    profile: ProfileId,
    token: CancellationToken,
    tasks: Vec<Box<dyn SpawnedTask>>,
}

struct RuntimeState {
    closed: bool,
    next_lease: LeaseId,
    leases: HashMap<LeaseId, LeaseState>,
}

struct RuntimeInner {
    endpoint: Arc<dyn NetworkEndpoint>,
    spawner: Arc<dyn TaskSpawner>,
    state: Mutex<RuntimeState>,
}

/// The process-singleton client internet runtime.
///
/// Cloning shares the same underlying endpoint, task spawner, and lease table;
/// it never constructs a second endpoint or runtime.
#[derive(Clone)]
pub struct RiotApplicationRuntime {
    inner: Arc<RuntimeInner>,
}

/// The process-singleton guard that constructs [`RiotApplicationRuntime`] once.
///
/// The first call to [`get_or_start`](RuntimeHost::get_or_start) invokes the
/// endpoint factory exactly once; later calls return a clone of the same
/// runtime without creating a second endpoint.
#[derive(Default)]
pub struct RuntimeHost {
    cell: Mutex<Option<RiotApplicationRuntime>>,
}

impl RuntimeHost {
    /// A host holding no runtime yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the single application runtime, constructing it on first call.
    ///
    /// Idempotent: the endpoint factory is invoked exactly once across the life
    /// of this host; subsequent calls return a clone of the existing runtime.
    pub fn get_or_start(
        &self,
        factory: &dyn EndpointFactory,
        spawner: Arc<dyn TaskSpawner>,
    ) -> RuntimeResult<RiotApplicationRuntime> {
        let mut cell = self.cell.lock().unwrap();
        if let Some(existing) = cell.as_ref() {
            return Ok(existing.clone());
        }
        let endpoint = factory.create_endpoint()?;
        let runtime = RiotApplicationRuntime {
            inner: Arc::new(RuntimeInner {
                endpoint,
                spawner,
                state: Mutex::new(RuntimeState {
                    closed: false,
                    next_lease: 0,
                    leases: HashMap::new(),
                }),
            }),
        };
        *cell = Some(runtime.clone());
        Ok(runtime)
    }
}

impl RiotApplicationRuntime {
    /// Acquire a revocable network lease for `profile`.
    pub fn acquire_profile_lease(&self, profile: ProfileId) -> RuntimeResult<ProfileLease> {
        let mut state = self.inner.state.lock().unwrap();
        if state.closed {
            return Err(RuntimeError::Closed);
        }
        let id = state.next_lease;
        state.next_lease += 1;
        state.leases.insert(
            id,
            LeaseState {
                profile,
                token: CancellationToken::new(),
                tasks: Vec::new(),
            },
        );
        Ok(ProfileLease {
            runtime: self.inner.clone(),
            id,
            released: false,
        })
    }

    /// The number of profile leases currently open.
    pub fn open_lease_count(&self) -> usize {
        self.inner.state.lock().unwrap().leases.len()
    }

    /// The profiles that currently hold an open lease.
    pub fn open_profiles(&self) -> Vec<ProfileId> {
        self.inner
            .state
            .lock()
            .unwrap()
            .leases
            .values()
            .map(|lease| lease.profile.clone())
            .collect()
    }

    /// Close the application runtime.
    ///
    /// Refused with [`RuntimeError::ProfileLeasesOutstanding`] while any profile
    /// lease is open. Once all leases are released, the endpoint is shut down
    /// (bounded). Idempotent thereafter.
    pub fn close(&self) -> RuntimeResult<()> {
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.closed {
                return Ok(());
            }
            if !state.leases.is_empty() {
                return Err(RuntimeError::ProfileLeasesOutstanding(state.leases.len()));
            }
            state.closed = true;
        }
        // Shut the endpoint down outside the state lock: the shutdown is bounded
        // and must not block lease bookkeeping.
        self.inner.endpoint.shutdown();
        Ok(())
    }
}

/// A revocable per-profile network lease.
///
/// Dropping or [`release`](ProfileLease::release)ing the lease cancels only this
/// profile's tasks and drains them; sibling leases are untouched.
pub struct ProfileLease {
    runtime: Arc<RuntimeInner>,
    id: LeaseId,
    released: bool,
}

impl ProfileLease {
    /// Spawn a background task scoped to this lease.
    pub fn spawn_task(&self, task: RuntimeFuture) -> RuntimeResult<()> {
        let mut state = self.runtime.state.lock().unwrap();
        if state.closed {
            return Err(RuntimeError::Closed);
        }
        let token = state
            .leases
            .get(&self.id)
            .ok_or(RuntimeError::LeaseReleased)?
            .token
            .clone();
        let handle = self.runtime.spawner.spawn(token, task);
        state
            .leases
            .get_mut(&self.id)
            .ok_or(RuntimeError::LeaseReleased)?
            .tasks
            .push(handle);
        Ok(())
    }

    /// Whether this lease is still active (not yet released).
    pub fn is_active(&self) -> bool {
        !self.released
    }

    /// Release this lease: cancel only its token and drain only its tasks.
    /// Idempotent.
    pub fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        // Detach this lease's state under the lock; a sibling lease's tasks stay
        // in the table untouched.
        let removed = self.runtime.state.lock().unwrap().leases.remove(&self.id);
        if let Some(lease) = removed {
            // Cancel THIS lease's token first so each drained task observes it,
            // then drain (join) every task. Done outside the state lock.
            lease.token.cancel();
            for task in lease.tasks {
                task.join();
            }
        }
    }
}

impl Drop for ProfileLease {
    fn drop(&mut self) {
        self.release();
    }
}

// ---------------------------------------------------------------------------
// Real (native) implementations. Not exercised by the unit tests below, which
// inject fakes; these exist so the crate genuinely owns one Tokio runtime and
// one iroh endpoint for `riot-ffi` (WU-012) to consume.
// ---------------------------------------------------------------------------

/// The real task spawner. Owns the ONE Tokio multi-thread runtime.
pub struct TokioTaskSpawner {
    rt: tokio::runtime::Runtime,
    join_timeout: Duration,
}

impl TokioTaskSpawner {
    /// Build the one Tokio runtime for this process.
    pub fn new() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            rt,
            join_timeout: Duration::from_secs(5),
        })
    }

    /// A handle onto the one Tokio runtime, e.g. for [`IrohEndpointFactory`].
    pub fn handle(&self) -> tokio::runtime::Handle {
        self.rt.handle().clone()
    }
}

impl TaskSpawner for TokioTaskSpawner {
    fn spawn(&self, token: CancellationToken, task: RuntimeFuture) -> Box<dyn SpawnedTask> {
        let join = self.rt.spawn(task);
        Box::new(TokioTask {
            handle: self.rt.handle().clone(),
            join: Some(join),
            token,
            timeout: self.join_timeout,
        })
    }
}

struct TokioTask {
    handle: tokio::runtime::Handle,
    join: Option<tokio::task::JoinHandle<()>>,
    token: CancellationToken,
    timeout: Duration,
}

impl SpawnedTask for TokioTask {
    fn join(mut self: Box<Self>) {
        if let Some(join) = self.join.take() {
            // Release cancels the token before draining; abort makes the drain
            // bounded even for a task that never polls the token.
            if self.token.is_cancelled() {
                join.abort();
            }
            let _ = self
                .handle
                .block_on(async { tokio::time::timeout(self.timeout, join).await });
        }
    }
}

/// The real endpoint factory. Binds one iroh endpoint via `riot-transport`.
pub struct IrohEndpointFactory {
    handle: tokio::runtime::Handle,
    close_timeout: Duration,
}

impl IrohEndpointFactory {
    /// Build a factory that binds on the given Tokio runtime handle (typically
    /// [`TokioTaskSpawner::handle`]).
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self {
            handle,
            close_timeout: Duration::from_secs(5),
        }
    }
}

impl EndpointFactory for IrohEndpointFactory {
    fn create_endpoint(&self) -> RuntimeResult<Arc<dyn NetworkEndpoint>> {
        let endpoint = self
            .handle
            .block_on(riot_transport::iroh::bind())
            .map_err(|e| RuntimeError::EndpointCreation(format!("{e:?}")))?;
        Ok(Arc::new(IrohNetworkEndpoint {
            endpoint,
            handle: self.handle.clone(),
            close_timeout: self.close_timeout,
        }))
    }
}

struct IrohNetworkEndpoint {
    endpoint: iroh::Endpoint,
    handle: tokio::runtime::Handle,
    close_timeout: Duration,
}

impl NetworkEndpoint for IrohNetworkEndpoint {
    fn shutdown(&self) {
        let endpoint = self.endpoint.clone();
        let _ = self
            .handle
            .block_on(async { tokio::time::timeout(self.close_timeout, endpoint.close()).await });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    // ---- Fakes: deterministic, no network, no real Tokio runtime ----

    #[derive(Default)]
    struct FakeEndpoint {
        shutdowns: AtomicU64,
    }
    impl NetworkEndpoint for FakeEndpoint {
        fn shutdown(&self) {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct FakeEndpointFactory {
        creates: AtomicU64,
        endpoint: Arc<FakeEndpoint>,
    }
    impl FakeEndpointFactory {
        fn new() -> Self {
            Self {
                creates: AtomicU64::new(0),
                endpoint: Arc::new(FakeEndpoint::default()),
            }
        }
        fn create_count(&self) -> u64 {
            self.creates.load(Ordering::SeqCst)
        }
    }
    impl EndpointFactory for FakeEndpointFactory {
        fn create_endpoint(&self) -> RuntimeResult<Arc<dyn NetworkEndpoint>> {
            self.creates.fetch_add(1, Ordering::SeqCst);
            Ok(self.endpoint.clone())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Ev {
        Spawned(u64),
        Drained { id: u64, cancelled: bool },
    }

    #[derive(Default)]
    struct FakeSpawner {
        next: AtomicU64,
        log: Arc<Mutex<Vec<Ev>>>,
    }
    impl FakeSpawner {
        fn log(&self) -> Arc<Mutex<Vec<Ev>>> {
            self.log.clone()
        }
    }
    impl TaskSpawner for FakeSpawner {
        fn spawn(&self, token: CancellationToken, _task: RuntimeFuture) -> Box<dyn SpawnedTask> {
            let id = self.next.fetch_add(1, Ordering::SeqCst);
            self.log.lock().unwrap().push(Ev::Spawned(id));
            Box::new(FakeTask {
                id,
                token,
                log: self.log.clone(),
            })
        }
    }
    struct FakeTask {
        id: u64,
        token: CancellationToken,
        log: Arc<Mutex<Vec<Ev>>>,
    }
    impl SpawnedTask for FakeTask {
        fn join(self: Box<Self>) {
            self.log.lock().unwrap().push(Ev::Drained {
                id: self.id,
                cancelled: self.token.is_cancelled(),
            });
        }
    }

    fn noop_task() -> RuntimeFuture {
        Box::pin(async {})
    }

    fn drained_ids(log: &[Ev]) -> Vec<u64> {
        log.iter()
            .filter_map(|e| match e {
                Ev::Drained { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    struct Harness {
        factory: FakeEndpointFactory,
        spawner: Arc<FakeSpawner>,
    }
    impl Harness {
        fn new() -> Self {
            Self {
                factory: FakeEndpointFactory::new(),
                spawner: Arc::new(FakeSpawner::default()),
            }
        }
        fn start(&self, host: &RuntimeHost) -> RiotApplicationRuntime {
            host.get_or_start(&self.factory, self.spawner.clone())
                .expect("start runtime")
        }
    }

    #[test]
    fn duplicate_construction_reuses_single_runtime_and_endpoint() {
        let h = Harness::new();
        let host = RuntimeHost::new();

        let rt1 = h.start(&host);
        let rt2 = h.start(&host);

        // Only ONE endpoint was ever created.
        assert_eq!(h.factory.create_count(), 1, "endpoint factory called once");
        // Both handles share the same underlying runtime/endpoint.
        assert!(
            Arc::ptr_eq(&rt1.inner, &rt2.inner),
            "duplicate construction returns the existing runtime"
        );
    }

    #[test]
    fn profile_lease_release_cancels_only_its_tasks() {
        let h = Harness::new();
        let host = RuntimeHost::new();
        let rt = h.start(&host);
        let log = h.spawner.log();

        let mut lease_a = rt
            .acquire_profile_lease(ProfileId::new("alice"))
            .expect("lease a");
        let lease_b = rt
            .acquire_profile_lease(ProfileId::new("bob"))
            .expect("lease b");

        lease_a.spawn_task(noop_task()).unwrap(); // id 0
        lease_a.spawn_task(noop_task()).unwrap(); // id 1
        lease_b.spawn_task(noop_task()).unwrap(); // id 2

        lease_a.release();

        let snapshot = log.lock().unwrap().clone();
        // Only lease A's tasks (0, 1) were drained, and each observed cancellation.
        assert_eq!(drained_ids(&snapshot), vec![0, 1]);
        for ev in &snapshot {
            if let Ev::Drained { cancelled, .. } = ev {
                assert!(*cancelled, "released lease cancels its tasks before drain");
            }
        }
        // Lease B survives untouched: its task (2) was NOT drained.
        assert!(
            !drained_ids(&snapshot).contains(&2),
            "sibling task survives"
        );
        assert!(lease_b.is_active());
        assert_eq!(rt.open_lease_count(), 1);
        assert_eq!(rt.open_profiles(), vec![ProfileId::new("bob")]);
    }

    #[test]
    fn task_drain_on_lease_release() {
        let h = Harness::new();
        let host = RuntimeHost::new();
        let rt = h.start(&host);
        let log = h.spawner.log();

        let mut lease = rt
            .acquire_profile_lease(ProfileId::new("carol"))
            .expect("lease");
        lease.spawn_task(noop_task()).unwrap(); // id 0
        lease.spawn_task(noop_task()).unwrap(); // id 1
        lease.spawn_task(noop_task()).unwrap(); // id 2

        lease.release();

        // Every spawned task was drained on release.
        let mut drained = drained_ids(&log.lock().unwrap());
        drained.sort_unstable();
        assert_eq!(drained, vec![0, 1, 2]);
    }

    #[test]
    fn application_close_rejected_while_lease_open_then_succeeds() {
        let h = Harness::new();
        let host = RuntimeHost::new();
        let rt = h.start(&host);
        let endpoint = h.factory.endpoint.clone();

        let mut lease = rt
            .acquire_profile_lease(ProfileId::new("dave"))
            .expect("lease");

        // Close is refused while the lease is open; endpoint is NOT shut down.
        assert_eq!(
            rt.close(),
            Err(RuntimeError::ProfileLeasesOutstanding(1)),
            "close refused with an open lease"
        );
        assert_eq!(endpoint.shutdowns.load(Ordering::SeqCst), 0);

        // After releasing the lease, close succeeds and shuts the endpoint down.
        lease.release();
        assert_eq!(rt.close(), Ok(()));
        assert_eq!(endpoint.shutdowns.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn endpoint_shutdown_is_bounded_and_idempotent() {
        let h = Harness::new();
        let host = RuntimeHost::new();
        let rt = h.start(&host);
        let endpoint = h.factory.endpoint.clone();

        // With no leases open, close completes (bounded) and shuts down once.
        assert_eq!(rt.close(), Ok(()));
        assert_eq!(endpoint.shutdowns.load(Ordering::SeqCst), 1);

        // A second close is a no-op: no second endpoint shutdown.
        assert_eq!(rt.close(), Ok(()));
        assert_eq!(endpoint.shutdowns.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn operations_after_close_are_rejected() {
        let h = Harness::new();
        let host = RuntimeHost::new();
        let rt = h.start(&host);
        rt.close().unwrap();

        assert!(matches!(
            rt.acquire_profile_lease(ProfileId::new("erin")),
            Err(RuntimeError::Closed)
        ));
    }

    #[test]
    fn double_release_is_idempotent() {
        let h = Harness::new();
        let host = RuntimeHost::new();
        let rt = h.start(&host);
        let log = h.spawner.log();

        let mut lease = rt
            .acquire_profile_lease(ProfileId::new("frank"))
            .expect("lease");
        lease.spawn_task(noop_task()).unwrap(); // id 0

        lease.release();
        lease.release(); // no-op

        // The task was drained exactly once despite the double release.
        assert_eq!(drained_ids(&log.lock().unwrap()), vec![0]);
        assert_eq!(rt.open_lease_count(), 0);
    }
}
