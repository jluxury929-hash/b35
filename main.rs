use ethers::{
    prelude::*,
    providers::{Provider, Ws},
    utils::parse_ether,
    abi::{Token, encode},
};
use ethers_flashbots::{BundleRequest, FlashbotsMiddleware};
use petgraph::{graph::{NodeIndex, UnGraph}, visit::EdgeRef};
use std::{sync::Arc, collections::HashMap, str::FromStr, net::TcpListener, io::Write, thread};
use colored::*;
use dotenv::dotenv;
use std::env;
use anyhow::{Result, anyhow};
use url::Url;
use log::{info, error};

// --- CONFIGURATION ---
const WETH_ADDR: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"; 

// --- ABIGEN (INTERFACES) ---
abigen!(
    IUniswapV2Pair,
    r#"[
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        function token0() external view returns (address)
        function token1() external view returns (address)
    ]"#
);

abigen!(
    ApexOmega,
    r#"[ function execute(uint256 mode, address token, uint256 amount, bytes calldata strategy) external payable ]"#
);

#[derive(Clone, Copy, Debug)]
struct PoolEdge {
    pair_address: Address,
    token_0: Address,
    token_1: Address,
    reserve_0: U256,
    reserve_1: U256,
    fee_numerator: u32,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    println!("{}", "╔════════════════════════════════════════════════════════╗".yellow());
    println!("{}", "║    ⚡ APEX OMEGA: RUST SINGULARITY (FINAL BUILD)      ║".yellow());
    println!("{}", "║    MODE: INFINITE RECURSION | ZERO-COPY | FLASHBOTS    ║".yellow());
    println!("{}", "╚════════════════════════════════════════════════════════╝".yellow());

    if let Err(e) = run().await {
        error!("FATAL CRASH: {:?}", e);
        // Keep process alive so logs can be read
        std::thread::sleep(std::time::Duration::from_secs(60));
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    validate_env()?;

    // CLOUD BOOT GUARD
    thread::spawn(|| {
        let listener = TcpListener::bind("0.0.0.0:8080").expect("Failed to bind port 8080");
        info!("Health Monitor Active on Port 8080");
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"HUNTING\",\"engine\":\"RUST-APEX\"}";
                stream.write_all(response.as_bytes()).unwrap_or_default();
            }
        }
    });

    let ws_url = env::var("WSS_URL").expect("Missing WSS_URL in env");
    let provider = Provider::<Ws>::connect(&ws_url).await?;
    let provider = Arc::new(provider);
    
    let wallet: LocalWallet = env::var("PRIVATE_KEY")?.parse()?;
    let chain_id = provider.get_chainid().await?.as_u64();
    let client = SignerMiddleware::new(provider.clone(), wallet.clone().with_chain_id(chain_id));
    let client = Arc::new(client);

    // Dummy signer for Flashbots auth if none provided
    let fb_signer: LocalWallet = "0000000000000000000000000000000000000000000000000000000000000001".parse()?;
    let fb_client = FlashbotsMiddleware::new(
        client.clone(),
        Url::parse("https://relay.flashbots.net")?,
        fb_signer, 
    );

    let executor_addr: Address = env::var("EXECUTOR_ADDRESS")?.parse()?;
    let executor = ApexOmega::new(executor_addr, client.clone());

    let mut graph = UnGraph::<Address, PoolEdge>::new_undirected();
    let mut node_map: HashMap<Address, NodeIndex> = HashMap::new();
    let mut pair_map: HashMap<Address, petgraph::graph::EdgeIndex> = HashMap::new();

    let pools = vec![
        "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc", 
    ];

    info!("Initializing Infinite Graph with {} pools...", pools.len());

    for pool_addr in pools {
        if let Ok(addr) = Address::from_str(pool_addr) {
            let pair = IUniswapV2Pair::new(addr, provider.clone());
            
            if let Ok((r0, r1, _)) = pair.get_reserves().call().await {
                // Fixed naming: token_0()
                let t0 = pair.token_0().call().await?;
                let t1 = pair.token_1().call().await?;

                let n0 = *node_map.entry(t0).or_insert_with(|| graph.add_node(t0));
                let n1 = *node_map.entry(t1).or_insert_with(|| graph.add_node(t1));

                let edge_idx = graph.add_edge(n0, n1, PoolEdge {
                    pair_address: addr, token_0: t0, token_1: t1, reserve_0: r0.into(), reserve_1: r1.into(), fee_numerator: 997,
                });
                pair_map.insert(addr, edge_idx);
            }
        }
    }

    info!("Engine Armed. Monitoring Mempool...");

    let filter = Filter::new().event("Sync(uint112,uint112)");
    let mut stream = provider.subscribe_logs(&filter).await?;

    while let Some(log) = stream.next().await {
        if let Some(edge_idx) = pair_map.get(&log.address) {
            if let Some(edge) = graph.edge_weight_mut(*edge_idx) {
                let r0 = U256::from_big_endian(&log.data[0..32]);
                let r1 = U256::from_big_endian(&log.data[32..64]);
                edge.reserve_0 = r0;
                edge.reserve_1 = r1;
            }

            let weth = Address::from_str(WETH_ADDR)?;
            if let Some(start) = node_map.get(&weth) {
                let amt_in = parse_ether("10")?; 
                
                // Recursion depth 4
                if let Some((profit, route)) = find_arb_recursive(&graph, *start, *start, amt_in, 4, vec![]) {
                    if profit > parse_ether("0.05")? {
                        info!("{} PROFIT: {} ETH | HOPS: {}", "💎".yellow().bold(), profit, route.len());
                        
                        let bribe = profit * 90 / 100;
                        let strategy_bytes = build_strategy(route, amt_in, bribe, executor_addr, &graph)?;

                        let mut tx = executor.execute(
                            U256::zero(), 
                            weth,
                            amt_in,
                            strategy_bytes
                        ).tx;

                        // Fill and Sign
                        client.fill_transaction(&mut tx, None).await.ok();
                        
                        if let Ok(signature) = client.signer().sign_transaction(&tx).await {
                             let rlp_signed_tx = tx.rlp_signed(&signature);
                             
                             let block = provider.get_block_number().await.unwrap_or_default();
                             let bundle = BundleRequest::new()
                                .push_transaction(rlp_signed_tx)
                                .set_block(block + 1)
                                .set_simulation_block(block)
                                .set_simulation_timestamp(0);

                             // Clone client for async move
                             let cl = fb_client.clone();
                             tokio::spawn(async move {
                                cl.send_bundle(&bundle).await.ok();
                             });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_env() -> Result<()> {
    let key = env::var("PRIVATE_KEY").map_err(|_| anyhow!("Missing PRIVATE_KEY"))?;
    if key.len() != 64 && !key.starts_with("0x") { return Err(anyhow!("Invalid Private Key Length")); }
    let exec = env::var("EXECUTOR_ADDRESS").map_err(|_| anyhow!("Missing EXECUTOR_ADDRESS"))?;
    if exec.len() != 42 { return Err(anyhow!("Invalid Contract Address Length")); }
    Ok(())
}

fn find_arb_recursive(
    graph: &UnGraph<Address, PoolEdge>,
    curr: NodeIndex,
    start: NodeIndex,
    amt: U256,
    depth: u8,
    mut path: Vec<(Address, Address)>
) -> Option<(U256, Vec<(Address, Address)>)> {
    if curr == start && path.len() > 1 {
        let initial = parse_ether("10").unwrap();
        return if amt > initial { Some((amt - initial, path)) } else { None };
    }
    if depth == 0 { return None; }

    for edge in graph.edges(curr) {
        let next = edge.target();
        if path.iter().any(|(a, _)| *a == *graph.node_weight(next).unwrap()) && next != start { continue; }
        
        let out = get_amount_out(amt, edge.weight(), curr, graph);
        if out.is_zero() { continue; }

        let mut next_path = path.clone();
        next_path.push((*graph.node_weight(curr).unwrap(), *graph.node_weight(next).unwrap()));
        
        if let Some(res) = find_arb_recursive(graph, next, start, out, depth - 1, next_path) {
            return Some(res);
        }
    }
    None
}

fn get_amount_out(amt_in: U256, edge: &PoolEdge, curr: NodeIndex, graph: &UnGraph<Address, PoolEdge>) -> U256 {
    let addr = graph.node_weight(curr).unwrap();
    // Fixed: Dereferencing *addr
    let (r_in, r_out) = if *addr == edge.token_0 { (edge.reserve_0, edge.reserve_1) } else { (edge.reserve_1, edge.reserve_0) };
    if r_in.is_zero() || r_out.is_zero() { return U256::zero(); }
    let amt_fee = amt_in * edge.fee_numerator;
    (amt_fee * r_out) / ((r_in * 1000) + amt_fee)
}

fn build_strategy(
    route: Vec<(Address, Address)>,
    init_amt: U256,
    bribe: U256,
    contract: Address,
    graph: &UnGraph<Address, PoolEdge>
) -> Result<Bytes> {
    let mut targets = Vec::new();
    let mut payloads = Vec::new();
    let mut curr_in = init_amt;

    let t_sig = [0xa9, 0x05, 0x9c, 0xbb]; 
    let s_sig = [0x02, 0x2c, 0x0d, 0x9f]; 

    for (i, (tin, tout)) in route.iter().enumerate() {
        // Fixed: Renamed inner vars to node_idx to avoid shadowing outer 'i'
        // Fixed: Dereferencing *tin and *tout
        let nin = graph.node_indices().find(|node_idx| *graph.node_weight(*node_idx).unwrap() == *tin).unwrap();
        let nout = graph.node_indices().find(|node_idx| *graph.node_weight(*node_idx).unwrap() == *tout).unwrap();
        let edge = &graph[graph.find_edge(nin, nout).unwrap()];

        if i == 0 {
            targets.push(*tin);
            let mut d = t_sig.to_vec();
            d.extend(ethers::abi::encode(&[Token::Address(edge.pair_address), Token::Uint(init_amt)]));
            payloads.push(Bytes::from(d));
        }

        let out = get_amount_out(curr_in, edge, nin, graph);
        // Fixed: Dereference *tin
        let (a0, a1) = if *tin == edge.token_0 { (U256::zero(), out) } else { (out, U256::zero()) };
        
        // Fixed: Using outer 'i' for logic
        let to = if i == route.len() - 1 { contract } else {
            let (n_next_in, n_next_out) = (nout, graph.node_indices().find(|node_idx| *graph.node_weight(*node_idx).unwrap() == route[i+1].1).unwrap());
            graph[graph.find_edge(n_next_in, n_next_out).unwrap()].pair_address
        };

        targets.push(edge.pair_address);
        let mut d = s_sig.to_vec();
        d.extend(ethers::abi::encode(&[Token::Uint(a0), Token::Uint(a1), Token::Address(to), Token::Bytes(vec![])]));
        payloads.push(Bytes::from(d));

        curr_in = out;
    }

    // Fixed: Explicit type casting for map closures
    let encoded = encode(&[
        Token::Array(targets.into_iter().map(|t| Token::Address(t)).collect()),
        Token::Array(payloads.into_iter().map(|b| Token::Bytes(b.to_vec())).collect()),
        Token::Uint(bribe),
    ]);

    Ok(Bytes::from(encoded))
}
