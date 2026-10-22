//! The Ask panel's pure core: what is asked, how the page is framed for the
//! model, how the answer is parsed, checked and paced, what each tab's thread
//! holds, and which page styles are admitted.
//!
//! No I/O, clocks, threads, async runtimes or logging. Time arrives as
//! milliseconds from the caller; page text arrives already read and is only
//! ever treated as data.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::dbg_macro, clippy::print_stderr, clippy::print_stdout)]
#![cfg_attr(
    not(test),
    deny(clippy::panic, clippy::unreachable, clippy::unwrap_used)
)]

pub mod actions;
pub mod citations;
pub mod complexity;
pub mod cost;
pub mod css;
pub mod outline;
pub mod pacer;
pub mod page;
pub mod policy;
pub mod prompt;
pub mod protocol;
pub mod suggest;
pub mod text;
pub mod thread;

pub use zephium_ipc::page_assist as ipc;
