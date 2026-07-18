// simulation-engine/src/main.rs
use bytemuck::{Pod, Zeroable, bytes_of_mut};
use tokio::sync::watch;
use tokio::time::{Duration, sleep};
use warp::Filter;
use warp::filters::ws::Message;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Agent {
    id: u32,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Practice project: WasmEdge Async Engine started!");

    //create pipeline
    //tx = transmitter
    //rx = receiver
    let initial_state = Agent {
        id: 1,
        x: 0.0,
        y: 0.0,
        vx: 1.0,
        vy: 1.0,
    };
    let (tx, mut rx) = watch::channel(initial_state);

    tokio::spawn(async move {
        let mut test_agent = initial_state;

        loop {
            test_agent.x += test_agent.vx;
            test_agent.y += test_agent.vy;
            if test_agent.x < 0.0 || test_agent.x > 100.0 {
                test_agent.vx *= -1.0;
            }
            if test_agent.y < 0.0 || test_agent.y > 100.0 {
                test_agent.vy *= -1.0;
            }

            let _ = tx.send(test_agent);
            sleep(Duration::from_millis(16)).await;
        }
    });

    println!("Waiting for connections...");
    //network loop

    let stream_route = warp::path("stream")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let rx = rx.clone();
            ws.on_upgrade(move |socket| handle_connection(socket, rx))
        });

    println!("Websocket listening on ws://0.0.0.0:8080/stream");
    warp::serve(stream_route).run(([0, 0, 0, 0], 8080)).await;
}

async fn handle_connection(mut ws: warp::ws::WebSocket, mut rx: watch::Receiver<Agent>) {
    use futures_util::SinkExt;

    println!("New client connected");

    loop {
        if rx.changed().await.is_ok() {
            let latest_data = *rx.borrow_and_update();

            let payload_bytes: &[u8] = bytemuck::bytes_of(&latest_data);

            let msg = warp::ws::Message::binary(payload_bytes);
            if ws.send(msg).await.is_err() {
                println!("Client disconnected");
                break;
            }
        }
    }
}
