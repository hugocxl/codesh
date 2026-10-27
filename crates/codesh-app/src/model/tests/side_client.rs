use super::*;

fn side_connections(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> usize {
    requests
        .try_iter()
        .filter(|(_, work)| matches!(work, Work::SideConnection(_)))
        .count()
}

#[gpui::test]
fn a_server_opens_one_side_connection_at_a_time(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        side_connections(&requests);
        let local = ServerId::local();
        assert!(model.side_client(local, cx).is_none(), "none is open yet");
        assert!(model.side_client(local, cx).is_none());
        model.connect_side_client(local, cx);
        assert_eq!(
            side_connections(&requests),
            1,
            "callers share the connection being opened"
        );
    });
}

#[gpui::test]
fn side_connection_installs_after_main_connection_becomes_ready(cx: &mut TestAppContext) {
    let directory = tempfile::Builder::new()
        .prefix("codesh-side-")
        .tempdir_in("/tmp")
        .expect("profile");
    let socket = directory.path().join("server.sock");
    servers::serve_remote(&socket).expect("server");
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(
            model.connection(ServerId::local()),
            ConnectionState::Connecting
        );
        model.connect_side_client(ServerId::local(), cx);
        assert!(model.error.is_none(), "connecting is not a disconnection");
        assert_eq!(side_connections(&requests), 0);
        model.receive(local(1, Update::Connected(vec![])), cx);
        assert!(model.ready(ServerId::local()));
    });
    let reply = requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::SideConnection(reply) => Some(reply),
            _ => None,
        })
        .expect("Ready retries the side connection");
    cx.run_until_parked();
    let client = Client::connect(&socket).expect("side handshake");
    reply.try_send(Ok(client)).expect("deliver side connection");
    wait(cx, &view, |model, _| {
        model.side_clients.client(ServerId::local()).is_some()
    })
    .expect("side connection installed");
    view.update(cx, |model, _| {
        model.disconnect_side_client(ServerId::local());
    });
}

#[gpui::test]
fn local_worker_installs_side_connection(cx: &mut TestAppContext) {
    let directory = tempfile::Builder::new()
        .prefix("codesh-worker-")
        .tempdir_in("/tmp")
        .expect("profile");
    let socket = directory.path().join("server.sock");
    servers::serve_remote(&socket).expect("server");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (updates, received) = async_channel::unbounded();
    boot.work = crate::boot::worker(
        ServerId::local(),
        crate::boot::Target::Local(socket),
        updates,
    )
    .expect("worker");
    boot.updates = received;
    boot.work.send((1, Work::Connect)).expect("connect");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    wait(cx, &view, |model, _| {
        model.side_clients.client(ServerId::local()).is_some()
    })
    .expect("side connection installed through local worker");
    view.update(cx, |model, _| {
        assert!(model.ready(ServerId::local()));
        assert!(
            model.error.is_none(),
            "startup must not report a disconnection"
        );
        model.disconnect_side_client(ServerId::local());
        model.stop_workers();
    });
}
