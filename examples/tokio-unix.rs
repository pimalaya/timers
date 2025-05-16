#![cfg(feature = "client")]
#![cfg(feature = "server")]

use std::{env, path::PathBuf, sync::Arc, time::Duration};

use io_stream::runtimes::tokio::handle;
use io_timer::{
    client::coroutines::{GetTimer, StartTimer},
    server::coroutines::HandleRequest,
    timer::{TimerConfig, TimerCycles, TimerEvent, TimerLoop},
    Timer,
};
use log::{debug, info, trace};
use tempdir::TempDir;
use tokio::{
    net::{UnixListener, UnixStream},
    spawn,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    time::sleep,
};

#[tokio::main]
async fn main() {
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "debug");
    }

    env_logger::init();

    let socket = match env::var("SOCKET") {
        Ok(path) => PathBuf::from(path),
        Err(_) => TempDir::new("timer").unwrap().into_path().join("socket"),
    };

    let timer = Arc::new(Mutex::new(Timer::new(TimerConfig {
        cycles: TimerCycles::from([("Work", 2).into(), ("Rest", 3).into()]),
        cycles_count: TimerLoop::Infinite,
    })));

    // used for receiving timer notifications
    let (tx, rx) = unbounded_channel();

    // used for client <-> server communication
    let listener = UnixListener::bind(&socket).unwrap();

    spawn_event_notifier(rx);
    spawn_timer_tick(timer.clone(), tx.clone());
    spawn_server(timer.clone(), tx.clone(), listener);

    sleep(Duration::from_secs(3)).await;

    debug!("connect to {}", socket.display());
    let mut stream = UnixStream::connect(socket).await.unwrap();

    let mut arg = None;
    let mut start = StartTimer::new();

    while let Err(io) = start.resume(arg.take()) {
        arg = Some(handle(&mut stream, io).await.unwrap());
    }

    sleep(Duration::from_secs(3)).await;

    let mut arg = None;
    let mut get = GetTimer::new();

    let timer = loop {
        match get.resume(arg.take()) {
            Ok(timer) => break timer,
            Err(io) => arg = Some(handle(&mut stream, io).await.unwrap()),
        }
    };

    debug!("{timer:#?}");
}

fn spawn_event_notifier(mut mpsc: UnboundedReceiver<TimerEvent>) {
    info!("start event notifier");
    spawn(async move {
        loop {
            let event = mpsc.recv().await.unwrap();
            debug!("received event {event:?}");
        }
    });
}

fn spawn_timer_tick(timer: Arc<Mutex<Timer>>, mpsc: UnboundedSender<TimerEvent>) {
    info!("start timer tick");
    spawn(async move {
        loop {
            let mut timer = timer.lock().await;
            let events = timer.update();
            debug!("timer: tick");
            trace!("{timer:?}");
            drop(timer);

            for event in events {
                mpsc.send(event).unwrap();
            }

            sleep(Duration::from_secs(1)).await;
        }
    });
}

fn spawn_server(timer: Arc<Mutex<Timer>>, mpsc: UnboundedSender<TimerEvent>, socket: UnixListener) {
    info!("start server");
    spawn(async move {
        let (mut stream, _) = socket.accept().await.unwrap();
        debug!("server: received unix connection");

        loop {
            let mut arg = None;
            let mut handler = HandleRequest::new();

            let events = loop {
                let mut timer = timer.lock().await;
                let res = handler.resume(&mut timer, arg.take());
                drop(timer);

                match res {
                    Ok(events) => break events,
                    Err(io) => arg = Some(handle(&mut stream, io).await.unwrap()),
                }
            };

            for event in events {
                mpsc.send(event).unwrap();
            }
        }
    });
}
