use clap::Parser;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::env;
use dotenv::dotenv;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Simple CLI for checking stock data
#[derive(Parser, Debug)]
#[command(name = "stockcli")]
#[command(about = "Fetch stock info via Finnhub.io", long_about = None)]
struct Args {
    /// Ticker symbol, e.g., AAPL
    ticker: String,
}

#[derive(Debug, Deserialize)]
struct Quote {
    c: f64, // current price
    h: f64, // high
    l: f64, // low
    pc: f64, // previous close
}

#[derive(Debug, Deserialize)]
struct CompanyProfile {
    name: String,
    ticker: String,
    exchange: String,
    ipo: String,
    finnhubIndustry: String,
    weburl: String,
}

#[derive(Debug, Deserialize)]
struct SymbolLookup {
    count: usize,
    result: Vec<SymbolSuggestion>,
}

#[derive(Debug, Deserialize)]
struct SymbolSuggestion {
    symbol: String,
    description: String,
}

fn main() {
    dotenv().ok(); // Load .env
    let args = Args::parse();
    let api_key = env::var("FINNHUB_API_KEY").expect("Missing FINNHUB_API_KEY in environment");

    let client = Client::new();
    let ticker = args.ticker.to_uppercase();

    // 1. Get quote
    let quote_url = format!("https://finnhub.io/api/v1/quote?symbol={}&token={}", ticker, api_key);
    let quote_response = client.get(&quote_url).send();

    match quote_response {
        Ok(resp) => {
            if let Ok(quote) = resp.json::<Quote>() {
                // Check if response is zeroed (invalid ticker)
                if quote.c == 0.0 {
                    suggest_similar(&client, &ticker, &api_key);
                    return;
                }

                // 2. Get company profile
                let profile_url = format!(
                    "https://finnhub.io/api/v1/stock/profile2?symbol={}&token={}",
                    ticker, api_key
                );
                let profile_response = client.get(&profile_url).send().unwrap();
                let profile: CompanyProfile = profile_response.json().unwrap();

                println!("\n📈 Stock Info for {}\n", profile.ticker);
                println!("Name:        {}", profile.name);
                println!("Exchange:    {}", profile.exchange);
                println!("Industry:    {}", profile.finnhubIndustry);
                println!("IPO Date:    {}", profile.ipo);
                println!("Website:     {}", profile.weburl);
                println!("\n💲 Price Info");
                println!("Current:     ${:.2}", quote.c);
                println!("High:        ${:.2}", quote.h);
                println!("Low:         ${:.2}", quote.l);
                println!("Prev Close:  ${:.2}", quote.pc);
            } else {
                suggest_similar(&client, &ticker, &api_key);
            }
        }
        Err(err) => {
            println!("❌ Failed to fetch stock info: {}", err);
        }
    }
}


fn suggest_similar(client: &Client, query: &str, api_key: &str) {
    println!("\n⚠️ Ticker not found. Searching for better matches...");

    let url = format!(
        "https://finnhub.io/api/v1/search?q={}&token={}",
        query, api_key
    );
    let resp = client.get(&url).send();

    match resp {
        Ok(r) => {
            if let Ok(result) = r.json::<SymbolLookup>() {
                if result.result.is_empty() {
                    println!("❌ No similar tickers found.");
                    return;
                }

                let matcher = SkimMatcherV2::default();
                let query_upper = query.to_uppercase();

                // Score each result and filter out bad tickers (e.g., OTC ones ending in 'F')
                let mut ranked: Vec<_> = result.result.iter()
                    .filter(|item| {
                        !item.symbol.ends_with('F') && item.symbol.len() <= 5 // Skip OTC/penny stocks
                    })
                    .filter_map(|item| {
                        matcher
                            .fuzzy_match(&item.symbol, &query_upper)
                            .map(|score| (item, score))
                    })
                    .collect();

                // Sort by best match
                ranked.sort_by(|a, b| b.1.cmp(&a.1));

                if ranked.is_empty() {
                    println!("❌ No similar tickers found.");
                    return;
                }

                println!("🔍 Did you mean:");
                for (item, _) in ranked.iter().take(5) {
                    println!("👉 {} — {}", item.symbol, item.description);
                }
            } else {
                println!("❌ Could not parse suggestions from Finnhub.");
            }
        }
        Err(e) => println!("❌ Error fetching suggestions: {}", e),
    }
}