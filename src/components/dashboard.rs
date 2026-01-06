use yew::prelude::*;
use yew::platform::spawn_local;
use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use serde::Deserialize;
use crate::utils::{format_with_commas, format_hashrate, format_difficulty};

#[derive(Clone, PartialEq, Debug, Deserialize)]
struct NetworkInfo {
    #[serde(rename = "virtualDaaScore")]
    virtual_daa_score: String,
    difficulty: f64,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
struct HashrateResponse {
    hashrate: f64,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
struct PriceResponse {
    price: f64,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
struct MarketCapResponse {
    marketcap: u64,
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
struct CoinSupply {
    #[serde(rename = "circulatingSupply")]
    circulating_supply: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct NetworkStats {
    pub height: String,
    pub hashrate: String,
    pub difficulty: String,
    pub supply: String,
    pub price_usd: f64,
    pub market_cap_usd: String,
}

#[derive(Properties, PartialEq)]
pub struct DashboardProps {
    pub balance: String,
    pub is_loading: bool,
    pub last_refreshed: String,
}

#[function_component(Dashboard)]
pub fn dashboard(props: &DashboardProps) -> Html {
    let stats = use_state(|| Option::<NetworkStats>::None);
    let stats_loading = use_state(|| true);
    let stats_error = use_state(|| false);

    let fetch_stats = {
        let stats = stats.clone();
        let stats_loading = stats_loading.clone();
        let stats_error = stats_error.clone();

        Callback::from(move |_| {
            let stats = stats.clone();
            let stats_loading = stats_loading.clone();
            let stats_error = stats_error.clone();

            spawn_local(async move {
                stats_loading.set(true);
                stats_error.set(false);

                let mut new_stats = NetworkStats {
                    height: "0".to_string(),
                    hashrate: "N/A".to_string(),
                    difficulty: "N/A".to_string(),
                    supply: "N/A".to_string(),
                    price_usd: 0.0,
                    market_cap_usd: "0".to_string(),
                };

                let mut success = false;

                if let Ok(resp) = Request::get("https://api.vecnoscan.org/info/network").send().await {
                    if resp.ok() {
                        if let Ok(info) = resp.json::<NetworkInfo>().await {
                            new_stats.height = format_with_commas(info.virtual_daa_score.parse::<u64>().unwrap_or(0));
                            new_stats.difficulty = format_difficulty(info.difficulty);
                            success = true;
                        }
                    }
                }

                if let Ok(resp) = Request::get("https://api.vecnoscan.org/info/hashrate?stringOnly=false").send().await {
                    if resp.ok() {
                        if let Ok(hr) = resp.json::<HashrateResponse>().await {
                            new_stats.hashrate = format_hashrate(hr.hashrate);
                            success = true;
                        }
                    }
                }

                if let Ok(resp) = Request::get("https://api.vecnoscan.org/info/price?stringOnly=false").send().await {
                    if resp.ok() {
                        if let Ok(pr) = resp.json::<PriceResponse>().await {
                            new_stats.price_usd = pr.price;
                            success = true;
                        }
                    }
                }

                if let Ok(resp) = Request::get("https://api.vecnoscan.org/info/marketcap?stringOnly=false").send().await {
                    if resp.ok() {
                        if let Ok(mc) = resp.json::<MarketCapResponse>().await {
                            new_stats.market_cap_usd = format_with_commas(mc.marketcap);
                            success = true;
                        }
                    }
                }

                if let Ok(resp) = Request::get("https://api.vecnoscan.org/info/coinsupply").send().await {
                    if resp.ok() {
                        if let Ok(supply_data) = resp.json::<CoinSupply>().await {
                            if let Ok(sompis) = supply_data.circulating_supply.parse::<u64>() {
                                let ve_amount = sompis / 100_000_000;
                                new_stats.supply = format_with_commas(ve_amount);
                                success = true;
                            }
                        }
                    }
                }

                if success {
                    stats.set(Some(new_stats));
                } else {
                    stats_error.set(true);
                }

                stats_loading.set(false);
            });
        })
    };

    {
        let fetch_stats = fetch_stats.clone();
        use_effect_with((), move |_| {
            fetch_stats.emit(());
            let interval = Interval::new(120_000, move || fetch_stats.emit(()));
            || drop(interval)
        });
    }

    html! {
        <div class="screen-container" role="main" aria-label="Vecno Wallet Dashboard">
            <div class="balance-container" aria-live="assertive">
                <h2>{"Wallet Balance"}</h2>
                <p class={classes!("balance", if props.is_loading { "loading" } else { "" })}>
                    {
                        if props.is_loading {
                            "Fetching balance..."
                        } else {
                            if props.balance.is_empty() || props.balance == "Balance: unavailable" {
                                "Preparing balance..."
                            } else {
                                {&props.balance}
                            }
                        }
                    }
                </p>
                <p class="last-updated" aria-live="polite">
                    { &props.last_refreshed }
                </p>
            </div>

            <div class="network-stats-section">
                <h3 class="section-title">{"Network Statistics"}</h3>

                if *stats_loading {
                    <div class="stats-grid loading-grid">
                        { (0..6).map(|_| html! {
                            <div class="stat-card skeleton">
                                <span class="stat-label skeleton-line"></span>
                                <span class="stat-value skeleton-line long"></span>
                            </div>
                        }).collect::<Html>() }
                    </div>
                    <p class="status loading-text">{"Loading network stats..."}</p>
                } else if *stats_error {
                    <p class="status error">{"Network stats unavailable (connection issue)"}</p>
                } else if let Some(s) = (*stats).as_ref() {
                    <div class="stats-grid">
                        <div class="stat-card">
                            <span class="stat-label">{"Block Height"}</span>
                            <span class="stat-value">{ &s.height }</span>
                        </div>
                        <div class="stat-card">
                            <span class="stat-label">{"Hashrate"}</span>
                            <span class="stat-value">{ &s.hashrate }</span>
                        </div>
                        <div class="stat-card">
                            <span class="stat-label">{"Difficulty"}</span>
                            <span class="stat-value">{ &s.difficulty }</span>
                        </div>
                        <div class="stat-card">
                            <span class="stat-label">{"Circulating Supply"}</span>
                            <span class="stat-value">{ &s.supply }{ "" }</span>
                        </div>
                        <div class="stat-card">
                            <span class="stat-label">{"Price (USD)"}</span>
                            <span class="stat-value">
                                {
                                    if s.price_usd > 0.0 {
                                        let formatted = format!("{:.8}", s.price_usd);
                                        let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
                                        format!("${}", trimmed)
                                    } else {
                                        "$0".to_string()
                                    }
                                }
                            </span>
                        </div>
                        <div class="stat-card">
                            <span class="stat-label">{"Market Cap (USD)"}</span>
                            <span class="stat-value">{ format!("${}", s.market_cap_usd) }</span>
                        </div>
                    </div>
                } else {
                    <p class="status">{"Network stats unavailable."}</p>
                }
            </div>
        </div>
    }
}