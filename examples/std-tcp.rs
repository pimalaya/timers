#![cfg(feature = "client")]
#![cfg(feature = "server")]

use std::{
    env,
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{sleep, spawn},
    time::Duration,
};

use io_stream::runtimes::std::handle;
use io_timer::{
    client::coroutines::{GetTimer, StartTimer},
    server::coroutines::HandleRequest,
    timer::{TimerConfig, TimerCycles, TimerEvent, TimerLoop},
    Timer,
};
use log::{debug, info, trace};

fn main() {
    if let Err(_) = env::var("RUST_LOG") {
        env::set_var("RUST_LOG", "debug");
    }

    env_logger::init();

    let host = match env::var("HOST") {
        Ok(host) => host,
        Err(_) => String::from("localhost"),
    };

    let port = match env::var("PORT") {
        Ok(port) => port.parse::<u16>().unwrap(),
        Err(_) => 0,
    };

    let timer = Arc::new(Mutex::new(Timer::new(TimerConfig {
        cycles: TimerCycles::from([("Work", 2).into(), ("Rest", 3).into()]),
        cycles_count: TimerLoop::Infinite,
    })));

    // used for receiving timer notifications
    let (tx, rx) = channel();

    // used for client <-> server communication
    let listener = TcpListener::bind((host.as_str(), port)).unwrap();
    let addr = listener.local_addr().unwrap();

    spawn_event_notifier(rx);
    spawn_timer_tick(timer.clone(), tx.clone());
    spawn_server(timer.clone(), tx.clone(), listener);

    sleep(Duration::from_secs(3));

    debug!("connect to {addr}");
    let mut stream = TcpStream::connect(addr).unwrap();

    let mut arg = None;
    let mut start = StartTimer::new();

    while let Err(io) = start.resume(arg.take()) {
        arg = Some(handle(&mut stream, io).unwrap());
    }

    sleep(Duration::from_secs(3));

    let mut arg = None;
    let mut get = GetTimer::new();

    let timer = loop {
        match get.resume(arg.take()) {
            Ok(timer) => break timer,
            Err(io) => arg = Some(handle(&mut stream, io).unwrap()),
        }
    };

    debug!("{timer:#?}");
}

fn spawn_event_notifier(mpsc: Receiver<TimerEvent>) {
    info!("start event notifier");
    spawn(move || loop {
        let event = mpsc.recv().unwrap();
        debug!("received event {event:?}");
    });
}

fn spawn_timer_tick(timer: Arc<Mutex<Timer>>, mpsc: Sender<TimerEvent>) {
    info!("start timer tick");
    spawn(move || loop {
        let mut timer = timer.lock().unwrap();
        let events = timer.update();
        debug!("timer: tick");
        trace!("{timer:?}");
        drop(timer);

        for event in events {
            mpsc.send(event).unwrap();
        }

        sleep(Duration::from_secs(1));
    });
}

fn spawn_server(timer: Arc<Mutex<Timer>>, mpsc: Sender<TimerEvent>, listener: TcpListener) {
    spawn(move || {
        info!("start server");
        let (mut stream, _) = listener.accept().unwrap();

        debug!("server received tcp connection");
        loop {
            let mut arg = None;
            let mut handler = HandleRequest::new();

            let events = loop {
                let mut timer = timer.lock().unwrap();
                let res = handler.resume(&mut timer, arg.take());
                drop(timer);

                match res {
                    Ok(events) => break events,
                    Err(io) => arg = Some(handle(&mut stream, io).unwrap()),
                }
            };

            for event in events {
                mpsc.send(event).unwrap();
            }
        }
    });
}
