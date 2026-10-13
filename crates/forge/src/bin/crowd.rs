//! crowd <game dir> [city forge, default DataPC_Damascus.forge]: a city's crowd as AC1 lays it out (`forge::crowd`):
//! its regions (area, density, the people they hold by builder and percent), its flows, and what failed to parse.
use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let game = std::path::Path::new(args.get(1).map_or(".", |s| s.as_str()));
    let forge_file = args.get(2).map_or("DataPC_Damascus.forge", |s| s.as_str());
    let t = std::time::Instant::now();
    let c = forge::crowd::load_city(game, forge_file)?;
    println!("{forge_file}: {} regions, {} duty regions, {} flows ({:.1} s)", c.cells.len(), c.duty.len(), c.flows.len(), t.elapsed().as_secs_f32());
    let mut people = 0.0;
    for cell in &c.cells {
        let Some((name, d)) = c.duty.get(&cell.duty) else {
            println!("  {:08x}: duty region {:08x} not found", cell.id, cell.duty);
            continue;
        };
        people += cell.area * d.density;
        println!(
            "  {name}: {:.0} m², {} triangles, density {} /m² (~{:.0} people), weight {} user {:?}",
            cell.area,
            cell.triangles.len(),
            d.density,
            cell.area * d.density,
            d.weight,
            cell.user
        );
        for f in &d.fractions {
            let who: Vec<&str> = f.ids.iter().filter_map(|id| c.builders.get(id).map(|s| s.as_str())).collect();
            println!("      {:5.1}% {}{:?}", f.percent, if f.clique { "clique " } else { "" }, who);
        }
    }
    let length: f32 =
        c.flows.iter().map(|(_, _, f)| f.points.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum::<f32>()).sum();
    let ids: std::collections::HashSet<u32> = c.flows.iter().map(|f| f.0).collect();
    let links: usize = c.flows.iter().map(|f| f.2.start.len() + f.2.end.len()).sum();
    let linked: usize = c.flows.iter().map(|f| f.2.start.iter().chain(&f.2.end).filter(|id| ids.contains(id)).count()).sum();
    println!("flows: {} ({length:.0} m), {linked}/{links} links to flows; about {people:.0} people over the city", c.flows.len());
    for (what, why) in &c.failed {
        println!("FAILED {what}: {why}");
    }
    Ok(())
}
