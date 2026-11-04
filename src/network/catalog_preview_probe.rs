//! Только ignored private preview: временные файлы игрока, мир лишь в памяти.
use super::*;

#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="private preview каталога263; только внешняя проба с двумя маркерами"]
async fn serve_java_26_3_catalog_probe() {
    serve_preview().await;
}

async fn serve_preview() {
    let version = mcsa_protocol::JavaVersion::V26_3;
    let port = 26026;
    let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
    assert!(Path::new("mcsa-catalog-preview263.marker").is_file());
    // Работаем только в отдельной временной папке, которую подготовила проба.
    assert!(Path::new("mcsa-gameplay-probe.marker").is_file());
    for folder in ["config", "world/playerdata", "skins"] { std::fs::create_dir_all(folder).unwrap(); }
    let properties=ServerProperties {online_mode:false,server_port:port,view_distance:2,simulation_distance:2,compression_threshold:-1,..Default::default()};
    let mut world=World::in_memory_with_catalog(catalog);
    for z in -24..24 {for x in -24..24 {world.set_block(x,63,z,blocks::state_by_name("stone").unwrap());}}
    world.set_spawn_position((0.5, 64.0, 0.5));
    let mut shared=Shared::new(world,World::in_memory_with_catalog(catalog),PathBuf::from("world/playerdata"),PathBuf::from("skins"),properties.clone(),
        ops::Ops::open(Path::new("config/ops.json")),skins::Settings {mojang:false,ely:false,..Default::default()});
    // Только private test: ни startup/config, ни публичный admission не меняются.
    assert_eq!(shared.catalog(), catalog);
    shared.settings.java_version=Some(version);
    shared.settings.auth_enabled=false;shared.settings.auth_premium=false;shared.settings.prefetch_chunks=0;
    let shared=Arc::new(shared);let properties=Arc::new(properties);
    network::packet::set_compression_threshold(-1);
    let listener=TcpListener::bind(("127.0.0.1",port)).await.unwrap();
    let (sender,mut shutdown)=watch::channel(false);
    console::start(Arc::clone(&shared),sender);tick::start(Arc::clone(&shared));
    println!("MCSA catalog preview263 active; world=in_memory; persistent_world=false");
    println!("MCSA gameplay probe ready on {port}");
    let mut tasks=tokio::task::JoinSet::new();
    loop {tokio::select!{
        accepted=listener.accept()=>{
            let (socket,addr)=accepted.unwrap();socket.set_nodelay(true).unwrap();
            let shared=Arc::clone(&shared);let properties=Arc::clone(&properties);
            tasks.spawn(async move {
                let result = handle_connection(socket, addr, properties, shared).await;
                if let Err(error) = result { eprintln!("MCSA probe connection: {error}"); }
            });
        }
        _=shutdown.changed()=>break,
    }}
    kick_everyone(&shared).await;tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    tasks.abort_all();
}
