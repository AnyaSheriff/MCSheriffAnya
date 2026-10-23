//! Отдельный ignored-сервер для реального клиента. В release такой точки входа нет.
use super::*;

#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="изолированная игровая сессия777, запуск только внешней пробой"]
async fn serve_java_26_3_probe() {
    serve(mcsa_protocol::JavaVersion::V26_3, 26026).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="изолированная игровая сессия340, запуск только внешней пробой"]
async fn serve_java_1_12_2_probe() {
    serve(mcsa_protocol::JavaVersion::V1_12_2, 26027).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="изолированная игровая сессия776, запуск только внешней пробой"]
async fn serve_java_26_2_probe() {
    serve(mcsa_protocol::JavaVersion::V26_2, 26028).await;
}

#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="частная сессия776 на основном263; только внешняя проба"]
async fn serve_java_26_2_primary_26_3_probe() {
    assert!(Path::new("mcsa-catalog-preview263.marker").is_file());
    serve(mcsa_protocol::JavaVersion::V26_2, 26028).await;
}

async fn serve(version: mcsa_protocol::JavaVersion, port: u16) {
    // Работаем только в отдельной временной папке, которую подготовила проба.
    assert!(Path::new("mcsa-gameplay-probe.marker").is_file());
    for folder in ["config", "world/playerdata", "skins"] { std::fs::create_dir_all(folder).unwrap(); }
    let properties=ServerProperties {online_mode:false,server_port:port,view_distance:2,simulation_distance:2,compression_threshold:-1,..Default::default()};
    let preview = Path::new("mcsa-catalog-preview263.marker").is_file();
    let catalog = crate::gameplay_catalog::Catalog::new(if preview { crate::gameplay_catalog::Profile::Preview26_3 } else { crate::gameplay_catalog::Profile::Java26_1_2 });
    let persistent = Path::new("mcsa-persistence-probe.marker").is_file();
    let mut world=if persistent {
        World::open_with_catalog("world", Some(1), crate::config::server_properties::WorldKind::Flat, catalog).unwrap()
    } else { World::in_memory_with_catalog(catalog) };
    for z in -24..24 {for x in -24..24 {world.set_block(x,63,z,blocks::state_by_name("stone").unwrap());}}
    world.set_spawn_position((0.5, 64.0, 0.5));
    let mut shared=Shared::new(world,World::in_memory_with_catalog(catalog),PathBuf::from("world/playerdata"),PathBuf::from("skins"),properties.clone(),
        ops::Ops::open(Path::new("config/ops.json")),skins::Settings {mojang:false,ely:false,..Default::default()});
    //776 — только клиент; игровые правила задаёт каталог основного сервера.
    shared.settings.java_version=Some(if preview { mcsa_protocol::JavaVersion::V26_3 } else if version == mcsa_protocol::JavaVersion::V26_2 { network::NATIVE_VERSION } else { version });
    shared.settings.auth_enabled=false;shared.settings.auth_premium=false;shared.settings.prefetch_chunks=0;
    let shared=Arc::new(shared);let properties=Arc::new(properties);
    network::packet::set_compression_threshold(-1);
    let listener=TcpListener::bind(("127.0.0.1",port)).await.unwrap();
    let (sender,mut shutdown)=watch::channel(false);
    console::start(Arc::clone(&shared),sender);tick::start(Arc::clone(&shared));
    println!("MCSA gameplay probe ready on {port}");
    let mut tasks=tokio::task::JoinSet::new();
    loop {tokio::select!{
        accepted=listener.accept()=>{
            let (socket,addr)=accepted.unwrap();socket.set_nodelay(true).unwrap();
            let shared=Arc::clone(&shared);let properties=Arc::clone(&properties);
            tasks.spawn(async move {
                let result = if version == mcsa_protocol::JavaVersion::V26_2 {
                    handle_connection_inner(socket, addr, properties, shared, true).await
                } else { handle_connection(socket, addr, properties, shared).await };
                if let Err(error) = result { eprintln!("MCSA probe connection: {error}"); }
            });
        }
        _=shutdown.changed()=>break,
    }}
    kick_everyone(&shared).await;tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    if persistent {
        shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap().save_all();
        println!("MCSA persistence probe saved world");
    }
    tasks.abort_all();
}
