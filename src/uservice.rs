// Channel-based non-blocking microservices without use of async
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2022-2025 by
//     Dr. Maxim Orlovsky <orlovsky@cyphernet.org>
//
// Copyright (C) 2022-2025 Cyphernet Labs,
//                         Institute for Distributed and Cognitive Systems,
//                         Lugano, Switzerland
// All rights reserved
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

use std::fmt::Display;
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use crossbeam_channel::{SendError, SendTimeoutError, Sender, TrySendError};

pub type UError = Box<dyn Display + Send>;
pub type UResult<T = ()> = Result<T, UError>;

#[derive(Clone, Debug)]
pub struct UResponder<T = (), E = UError>(Option<Sender<Result<T, E>>>);

impl<T, E> UResponder<T, E> {
    pub fn respond(&self, msg: Result<T, E>) -> Result<(), SendError<Result<T, E>>> {
        if let Some(sender) = &self.0 { sender.send(msg) } else { Ok(()) }
    }
}

#[derive(Clone)]
pub(crate) enum UMsg<Msg> {
    Msg(Msg),
    Terminate,
}

#[derive(Clone, Debug)]
pub struct UErrorMsg {
    pub service: String,
    pub error: String,
}

pub trait UService: Send + 'static {
    type Msg: Send;
    type Error: Display + Sync;
    const NAME: &'static str;

    fn tick(&mut self) -> Result<(), Self::Error> {
        // By default, do nothing
        Ok(())
    }
    fn process(&mut self, msg: Self::Msg) -> Result<ControlFlow<u8>, Self::Error>;
    fn terminate(&mut self);
    fn monitor(&self) -> Option<&USender<UErrorMsg>> { None }

    fn error(&self, context: &str, err: impl Display) {
        self.error_sender().report(context, err.to_string())
    }

    fn error_brief(&self, err: impl Display) { self.error_sender().report_brief(err.to_string()) }

    fn error_sender(&self) -> UErrorSender {
        UErrorSender {
            sender: self.monitor().cloned(),
            service_name: Self::NAME,
        }
    }

    fn set_self_sender(&mut self, _sender: USender<Self::Msg>) {
        // By default, do nothing
    }
    fn self_sender(&self) -> USender<Self::Msg> {
        // By default, panic
        panic!("the sender was not set");
    }
}

pub struct UErrorSender {
    sender: Option<USender<UErrorMsg>>,
    service_name: &'static str,
}

impl UErrorSender {
    pub fn report(&self, context: &str, err: impl ToString) {
        self.report_brief(format!("{context} - {}", err.to_string()))
    }

    pub fn report_brief(&self, err: impl ToString) {
        #[cfg(feature = "log")]
        {
            let error = err.to_string();
            log::error!(target: self.service_name, "{error}");

            let Some(sender) = &self.sender else {
                return;
            };
            if sender
                .send(UErrorMsg {
                    service: self.service_name.to_string(),
                    error,
                })
                .is_err()
            {
                log::error!(target: self.service_name, "Broken monitor channel");
            }
        }
        #[cfg(feature = "stderr")]
        eprintln!("Error in {}: {}", self.service_name, err.to_string());
    }
}

#[derive(Clone, Debug)]
pub struct USender<Msg>(pub(crate) Sender<UMsg<Msg>>);

impl<Msg> USender<Msg> {
    fn convert_timeout_error(err: SendTimeoutError<UMsg<Msg>>) -> SendTimeoutError<Msg> {
        match err {
            SendTimeoutError::Timeout(UMsg::Msg(msg)) => SendTimeoutError::Timeout(msg),
            SendTimeoutError::Disconnected(UMsg::Msg(msg)) => SendTimeoutError::Disconnected(msg),
            SendTimeoutError::Timeout(UMsg::Terminate)
            | SendTimeoutError::Disconnected(UMsg::Terminate) => {
                unreachable!()
            }
        }
    }

    pub fn send(&self, msg: Msg) -> Result<(), SendError<Msg>> {
        self.0.send(UMsg::Msg(msg)).map_err(|SendError(msg)| match msg {
            UMsg::Msg(msg) => SendError(msg),
            UMsg::Terminate => unreachable!(),
        })
    }

    pub fn try_send(&self, msg: Msg) -> Result<(), TrySendError<Msg>> {
        self.0.try_send(UMsg::Msg(msg)).map_err(|err| match err {
            TrySendError::Full(UMsg::Msg(msg)) => TrySendError::Full(msg),
            TrySendError::Disconnected(UMsg::Msg(msg)) => TrySendError::Disconnected(msg),
            TrySendError::Full(UMsg::Terminate) | TrySendError::Disconnected(UMsg::Terminate) => {
                unreachable!()
            }
        })
    }

    pub fn send_timeout(&self, msg: Msg, timeout: Duration) -> Result<(), SendTimeoutError<Msg>> {
        self.0.send_timeout(UMsg::Msg(msg), timeout).map_err(Self::convert_timeout_error)
    }

    pub fn send_deadline(&self, msg: Msg, deadline: Instant) -> Result<(), SendTimeoutError<Msg>> {
        self.0.send_deadline(UMsg::Msg(msg), deadline).map_err(Self::convert_timeout_error)
    }

    #[inline]
    pub fn is_empty(&self) -> bool { self.0.is_empty() }

    #[inline]
    pub fn is_full(&self) -> bool { self.0.is_full() }

    #[inline]
    pub fn len(&self) -> usize { self.0.len() }

    #[inline]
    pub fn capacity(&self) -> Option<usize> { self.0.capacity() }
}
