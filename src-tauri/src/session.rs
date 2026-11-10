use crate::backend::{Error, Event, Message, Operation, Outcome, Request};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
pub enum Failure {
    Backend(Error),
    Disconnected { message: String, sent: bool },
}

pub enum Notification {
    Event(Event),
    Disconnected(String),
}

type Reply = Result<Value, Error>;
type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Reply>>>>;

pub struct Session {
    child: Child,
    input: Option<ChildStdin>,
    pending: Pending,
    sequence: AtomicU64,
    disconnected: Arc<Mutex<Option<String>>>,
}

impl Session {
    pub fn spawn(
        mut command: Command,
        notify: impl Fn(Notification) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("バックエンドの入力を開けません")?;
        let output = child
            .stdout
            .take()
            .ok_or("バックエンドの出力を開けません")?;
        let stderr = child
            .stderr
            .take()
            .ok_or("バックエンドの診断を開けません")?;
        let diagnostic = Arc::new(Mutex::new(String::new()));
        let errors = Arc::clone(&diagnostic);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    break;
                };
                if let Ok(mut message) = errors.lock() {
                    *message = line;
                }
            }
        });
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let disconnected = Arc::new(Mutex::new(None));
        let waiting = Arc::clone(&pending);
        let status = Arc::clone(&disconnected);
        thread::spawn(move || {
            let mut failure =
                "バックエンドとの接続が切れました。下書きを保持して再接続してください".to_owned();
            for line in BufReader::new(output).lines() {
                let message = line.map_err(|e| e.to_string()).and_then(|line| {
                    serde_json::from_str::<Message>(&line).map_err(|e| e.to_string())
                });
                match message {
                    Ok(Message::Response {
                        id: Some(id),
                        outcome,
                    }) => {
                        if let Ok(mut replies) = waiting.lock() {
                            if let Some(reply) = replies.remove(&id) {
                                let value = match outcome {
                                    Outcome::Success { result } => Ok(result),
                                    Outcome::Failure { error } => Err(error),
                                };
                                let _ = reply.send(value);
                            }
                        }
                    }
                    Ok(Message::Event { event }) => notify(Notification::Event(event)),
                    Ok(Message::Response { id: None, .. }) => {
                        failure = "バックエンドの応答IDが不正です".into();
                        break;
                    }
                    Err(error) => {
                        failure = format!("バックエンドの応答を読めません: {error}");
                        break;
                    }
                }
            }
            if let Ok(diagnostic) = diagnostic.lock() {
                if !diagnostic.is_empty() {
                    failure.push_str(&format!(": {diagnostic}"));
                }
            }
            if let Ok(mut status) = status.lock() {
                *status = Some(failure.clone());
            }
            if let Ok(mut replies) = waiting.lock() {
                replies.clear();
            }
            notify(Notification::Disconnected(failure));
        });
        Ok(Self {
            child,
            input: Some(input),
            pending,
            sequence: AtomicU64::new(1),
            disconnected,
        })
    }

    pub fn request(&mut self, operation: Operation) -> Result<Value, Failure> {
        if let Some(message) = self
            .disconnected
            .lock()
            .map_err(|e| Failure::Disconnected {
                message: e.to_string(),
                sent: false,
            })?
            .clone()
        {
            return Err(Failure::Disconnected {
                message,
                sent: false,
            });
        }
        let id = self.sequence.fetch_add(1, Ordering::Relaxed);
        let (sender, reply) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|e| Failure::Disconnected {
                message: e.to_string(),
                sent: false,
            })?
            .insert(id, sender);
        let encoded =
            serde_json::to_vec(&Request { id, operation }).map_err(|e| Failure::Disconnected {
                message: e.to_string(),
                sent: false,
            })?;
        let result = self
            .input
            .as_mut()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "接続がありません"))
            .and_then(|input| {
                input
                    .write_all(&encoded)
                    .and_then(|()| input.write_all(b"\n"))
                    .and_then(|()| input.flush())
            });
        if let Err(error) = result {
            self.pending
                .lock()
                .ok()
                .map(|mut pending| pending.remove(&id));
            let message = error.to_string();
            if let Ok(mut status) = self.disconnected.lock() {
                *status = Some(message.clone());
            }
            self.input.take();
            return Err(Failure::Disconnected {
                message,
                sent: true,
            });
        }
        match reply.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(Failure::Backend(error)),
            Err(error) => {
                self.pending
                    .lock()
                    .ok()
                    .map(|mut pending| pending.remove(&id));
                let message = self.disconnected.lock().ok().and_then(|status| status.clone()).unwrap_or_else(|| format!("応答を確認できません。保存を自動再送せず、再接続後に内容を確認してください: {error}"));
                if let Ok(mut status) = self.disconnected.lock() {
                    *status = Some(message.clone());
                }
                self.input.take();
                Err(Failure::Disconnected {
                    message,
                    sent: true,
                })
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.input.take();
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn disconnected_backend_does_not_receive_a_second_request() {
        let mut command = Command::new("sh");
        command.args(["-c", "read request; exit 0"]);
        let mut session = Session::spawn(command, |_| {}).unwrap();
        assert!(matches!(
            session.request(Operation::ListRepos),
            Err(Failure::Disconnected { sent: true, .. })
        ));
        assert!(matches!(
            session.request(Operation::ListRepos),
            Err(Failure::Disconnected { sent: false, .. })
        ));
    }
}
