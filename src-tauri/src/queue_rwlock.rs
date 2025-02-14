use std::sync::mpsc::{channel, Sender};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread;

// 自定义的包装结构体
pub struct QueuedRwLock<T> {
    inner: RwLock<T>,
    tx: Sender<()>,
}

impl<T> QueuedRwLock<T> {
    // 构造函数
    pub fn new(data: T) -> Self {
        let (tx, rx) = channel();
        // 初始化时发送一个信号，表示队列初始为空
        tx.send(()).unwrap();
        let tx_clone = tx.clone();
        // 启动一个单独的线程来处理队列逻辑
        thread::spawn(move || {
            let _ = rx.recv();
            loop {
                if let Ok(()) = rx.recv() {
                    let _ = tx_clone.send(());
                }
            }
        });
        QueuedRwLock {
            inner: RwLock::new(data),
            tx,
        }
    }

    // 获取读锁
    pub fn read(&self) -> QueuedRwLockReadGuard<T> {
        // 等待队列中的信号
        self.tx.send(()).unwrap();
        let guard = self.inner.read().unwrap();
        QueuedRwLockReadGuard {
            inner: guard,
            tx: self.tx.clone(),
        }
    }

    // 获取写锁
    pub fn write(&self) -> QueuedRwLockWriteGuard<T> {
        // 等待队列中的信号
        self.tx.send(()).unwrap();
        let guard = self.inner.write().unwrap();
        QueuedRwLockWriteGuard {
            inner: guard,
            tx: self.tx.clone(),
        }
    }
}

// 自定义的读锁守卫
pub struct QueuedRwLockReadGuard<'a, T> {
    inner: RwLockReadGuard<'a, T>,
    tx: Sender<()>,
}

impl<'a, T> Drop for QueuedRwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        // 释放读锁时发送信号，允许下一个操作进入队列
        self.tx.send(()).unwrap();
    }
}

impl<'a, T> std::ops::Deref for QueuedRwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// 自定义的写锁守卫
pub struct QueuedRwLockWriteGuard<'a, T> {
    inner: RwLockWriteGuard<'a, T>,
    tx: Sender<()>,
}

impl<'a, T> Drop for QueuedRwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        // 释放写锁时发送信号，允许下一个操作进入队列
        self.tx.send(()).unwrap();
    }
}

impl<'a, T> std::ops::Deref for QueuedRwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> std::ops::DerefMut for QueuedRwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// fn main() {
//     let queued_rwlock = Arc::new(QueuedRwLock::new(vec![models::PartialDownloadTask]));

//     let mut handles = vec![];

//     // 创建多个读线程
//     for _ in 0..3 {
//         let queued_rwlock = queued_rwlock.clone();
//         let handle = thread::spawn(move || {
//             let guard = queued_rwlock.read();
//             println!("Read value: {:?}", guard.len());
//         });
//         handles.push(handle);
//     }

//     // 创建一个写线程
//     let queued_rwlock = queued_rwlock.clone();
//     let handle = thread::spawn(move || {
//         let mut guard = queued_rwlock.write();
//         guard.push(models::PartialDownloadTask);
//         println!("Written value: {:?}", guard.len());
//     });
//     handles.push(handle);

//     // 等待所有线程完成
//     for handle in handles {
//         handle.join().unwrap();
//     }
// }
