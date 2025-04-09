// Channel-based non-blocking microservices without use of async
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2022-2025 by
//     Dr. Maxim Orlovsky <orlovsky@cyphernet.org>
//
// Copyright (C) 2022-2025 Cyphernet Labs, InDCS, Switzerland. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::ops::ControlFlow;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{RecvTimeoutError, Sender};

use crate::uservice::UMsg;
use crate::{USender, UService};

#[derive(Debug)]
pub struct UThread<S: UService> {
    thread: Option<JoinHandle<()>>,
    sender: Sender<UMsg<S::Msg>>,
}

impl<S: UService> UThread<S> {
    pub fn new(mut service: S, ticks: Option<Duration>) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        service.set_self_sender(USender(sender.clone()));
        let thread = thread::spawn(move || {
            loop {
                let recv = || {
                    if let Some(timeout) = ticks {
                        receiver.recv_timeout(timeout)
                    } else {
                        receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
                    }
                };
                let msg = match recv() {
                    Ok(UMsg::Msg(msg)) => msg,
                    Ok(UMsg::Terminate) => {
                        #[cfg(feature = "log")]
                        log::debug!(target: S::NAME, "got terminate command");
                        service.terminate();
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        #[cfg(feature = "log")]
                        log::trace!(target: S::NAME, "timed out, restarting the event loop");
                        if let Err(err) = service.tick() {
                            service.error("service tick error", err)
                        };
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        #[cfg(feature = "log")]
                        log::error!(target: S::NAME, "service channel got disconnected");
                        service.error("channel to the service is broken", "disconnected");
                        break;
                    }
                };
                match service.process(msg) {
                    Err(err) => {
                        service.error("service process error", err);
                    }
                    Ok(ControlFlow::Break(code)) => {
                        if code == 0 {
                            #[cfg(feature = "log")]
                            log::info!(target: S::NAME, "thread is stopping on service request");
                        } else {
                            #[cfg(feature = "log")]
                            log::debug!(target: S::NAME, "stopping thread due to status {code} returned from the service");
                        }
                        service.terminate();
                        break;
                    }
                    Ok(ControlFlow::Continue(())) => {}
                }
            }
            #[cfg(feature = "log")]
            log::info!(target: S::NAME, "thread is stopped");
        });

        Self { thread: Some(thread), sender }
    }

    pub fn sender(&self) -> USender<S::Msg> { USender(self.sender.clone()) }

    pub fn join(&mut self) -> thread::Result<()> {
        if let Some(thread) = self.thread.take() {
            return thread.join().inspect_err(|_| {
                #[cfg(feature = "log")]
                log::error!(target: S::NAME, "unable to complete thread")
            });
        }
        Ok(())
    }
}

impl<S: UService> Drop for UThread<S> {
    fn drop(&mut self) {
        #[cfg(feature = "log")]
        log::debug!(target: S::NAME, "ordering service to terminate");
        self.sender.send(UMsg::Terminate).unwrap_or_else(|err| {
            panic!("unable to send terminate command to the {} thread: {err}", S::NAME)
        });
        if let Some(thread) = self.thread.take() {
            #[cfg(feature = "log")]
            log::info!(target: S::NAME, "waiting for the service thread to complete");
            thread
                .join()
                .unwrap_or_else(|err| panic!("unable to join the {} thread: {err:?}", S::NAME))
        }
    }
}
