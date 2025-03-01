use std::{collections::{HashMap, VecDeque}, sync::Arc, time::Duration};

use async_trait::async_trait;
use tokio::{sync::RwLock, task::AbortHandle};
use tower_sessions::{cookie::time::OffsetDateTime, session::{Id, Record}, session_store, SessionStore};

/// Why? The tower_session MemoryStore by default does not delete entries in time. Which may cause memory leak over long run.
/// Java's auto session management has no memory leak issue. Rust Axum should not have that issue, too!
/// 
/// The `ExpiringMemoryStore` automatically deletes object that is expired
/// The data is stored internally as Arc<RwLock<HashMap<Id, Record>>
/// The expiry is tracked by the ascending VecDequeue using creation time
/// 
/// Upon create, a background tokio thread is immediately started to delete expired session automatically every 5 seconds.
/// Upon dropping the `ExpiringMemoryStore`, the background tokio thread is automatically cancelled.

/// #Example
/// ```rust,no_run
/// use axum::routing::get;
/// use axum::Router;
/// use std::net::SocketAddr;
/// use tower_sessions::{cookie::time::{self, Duration, OffsetDateTime}, Expiry, Session, SessionManagerLayer};
/// 
/// #[tokio::main]
/// async fn main() {
///   let session_store = tower_sessions_auto_memory_store::ExpiringMemoryStore::new();
///   let session_layer = SessionManagerLayer::new(session_store)
///      .with_secure(false)
///      .with_expiry(Expiry::OnInactivity(Duration::seconds(1800)));
///   let app = Router::new()
///      .route("/debug/test", get(root))
///      .layer(session_layer);
///    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
///    println!("listening on {}", addr);
///    axum_server::bind(addr)
///      .serve(app.into_make_service())
///      .await
///      .unwrap();
/// }
/// 
/// async fn root() -> &'static str {
///   "Hello, World!"
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ExpiringMemoryStore {
    data: Arc<RwLock<HashMap<Id, Record>>>,
    order: Arc<RwLock<VecDeque<(Id, OffsetDateTime)>>>,
    handle: Option<AbortHandle>,
}

/// Upon dropping, the background thread will be killed.
impl Drop for ExpiringMemoryStore {
    fn drop(&mut self) {
        let hdl = &self.handle;
        match hdl {
            None => {
            },
            Some(inner) => {
                inner.abort();
            }
        }
    }
}

impl ExpiringMemoryStore {
    /// Create a new ExpiringMemoryStore
    pub fn new() -> Self {
        let data = Default::default();
        let order = Default::default();
        let mut result = ExpiringMemoryStore {
            data: Arc::clone(&data),
            handle: None,
            order: Arc::clone(&order),
        };
        let handle = tokio::spawn( async move {
            loop {
                let mut session_map = data.write().await;
                let mut order_list = order.write().await;
                let mut to_delete = Vec::new();
                loop {
                    let front = order_list.front();
                    if let Some((id, offset)) = front {
                        if is_expired(*offset) {
                            to_delete.push(id.clone());
                            order_list.pop_front();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                for id in &to_delete {
                    let remove_result = session_map.remove(&id);
                    if let Some(_) = remove_result {
                    } else {
                    }
                }
                //println!("Dropped {} sessions. Remaining {}:{}", to_delete.len(), order_list.len(), session_map.len());
                drop(order_list);
                drop(session_map);
                tokio::time::sleep(Duration::from_millis(5000)).await;
            }
        });
        let hdl1 = handle.abort_handle();
        result.handle.replace(hdl1);
        return result;
    }
}

#[async_trait]
impl SessionStore for ExpiringMemoryStore {
    async fn save(&self, record: &Record) ->  session_store::Result<()> {
        let mut store_guard = self.data.write().await;
        store_guard.insert(record.id, record.clone());
        Ok(())
    }
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let mut store_guard = self.data.write().await;
        while store_guard.contains_key(&record.id) {
            // Session ID collision mitigation.
            record.id = Id::default();
        }
        store_guard.insert(record.id, record.clone());
        let mut order_guard = self.order.write().await;
        order_guard.push_back((record.id.clone(), record.expiry_date.clone()));
        drop(order_guard);
        Ok(())
    }
    async fn load(&self,session_id: &Id) ->  session_store::Result<Option<Record>>{
        Ok(self
            .data
            .read()
            .await
            .get(session_id)
            .filter(|Record { expiry_date, .. }| is_active(*expiry_date))
            .cloned())
    }

    async fn delete(&self,session_id: &Id) ->  session_store::Result<()> {
        let mut w = self.data.write().await;
        w.remove(session_id);
        Ok(())
    }
}

fn is_active(expiry_date: OffsetDateTime) -> bool {
    !is_expired(expiry_date)
}

fn is_expired(expiry_date: OffsetDateTime) -> bool {
    expiry_date + Duration::from_secs(60) < OffsetDateTime::now_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn it_works() {
        let store = ExpiringMemoryStore::new();
        drop(store);
        assert_eq!(1, 1)
    }
}
