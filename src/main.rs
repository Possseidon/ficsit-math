mod classes;

use std::{collections::BTreeSet, env, io::stdin, path::Path};

use classes::{parse_rational, Classes, Item, ItemAmounts, StackSize};
use dotenvy::dotenv;
use num_rational::Rational64;
use num_traits::{One, ToPrimitive, Zero};

const HIGHLIGHT: &str = "\x1b[38;2;173;216;230m";
const TEXT: &str = "\x1b[94m";

const ERROR: &str = "\x1b[31m";
const WARN: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

const RECIPE: &str = "\x1b[36m";
const MACHINE: &str = "\x1b[94m";
const MACHINE_IN: &str = "\x1b[38;2;254;178;104m";
const MACHINE_OUT: &str = "\x1b[38;2;91;229;193m";
const QUANTITY: &str = "\x1b[32m";
const OVERCLOCKING: &str = "\x1b[95m";

const ITEM: &str = "\x1b[90m";
const THROUGHPUT: &str = "\x1b[33m";

fn main() {
    println!(
        "{TEXT}{} {HIGHLIGHT}{}{RESET}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    dotenv().ok();
    let classes = Classes::load(Path::new(
        env::var("SATISFACTORY_DOCS_PATH")
            .expect(r"SATISFACTORY_DOCS_PATH should be set (to e.g. .../Satisfactory/CommunityResources/docs/en-US.json)")
            .as_str(),
    ));

    // print all ingredients/products that don't exist
    let missing: BTreeSet<_> = classes
        .recipes
        .values()
        .flat_map(|recipe| {
            recipe
                .ingredients
                .0
                .iter()
                .chain(&recipe.product.0)
                .filter(|(item, _)| !classes.items.contains_key(item))
                .map(|(item, _)| item)
        })
        .collect();
    for item in missing {
        println!("{WARN}item {ITEM}{item:?}{WARN} was not found{RESET}");
    }

    println!();
    println!("{HIGHLIGHT}Welcome, Pioneer!{RESET}");
    println!();
    println!("{TEXT}FICSIT Inc. empowers you with state-of-the-art technology to optimize your factory planning and production.{RESET}");
    println!("{TEXT}To get started, simply enter the number of items you want to produce per minute, followed by the item name.{RESET}");
    println!();
    println!("{HIGHLIGHT}For example: {THROUGHPUT}60 {ITEM}Iron Plate{RESET}");
    println!();
    println!("{TEXT}Use negative amounts to find recipes with that item as an ingredient rather than as a product.{RESET}");
    println!("{TEXT}Multiply or divide the ratio of the last prompt by typing {THROUGHPUT}* 2 {TEXT}or {THROUGHPUT}/ 3{TEXT}{RESET}");
    println!();
    println!("{HIGHLIGHT}Exit via Ctrl+C or Ctrl+Z{RESET}");
    println!();

    let mut last = None;
    for line in stdin().lines() {
        let line = line.unwrap();
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        let multiplier = line
            .strip_prefix("*")
            .map(|multiplier| parse_rational(multiplier.trim_start()))
            .or_else(|| {
                line.strip_prefix("/")
                    .map(|divisor| parse_rational(divisor.trim_start()).recip())
            });

        let (mut target_amount_per_min, item_name) = if let Some(multiplier) = multiplier {
            if let Some((target_amount_per_min, item_name)) = &mut last {
                *target_amount_per_min *= multiplier;
                (*target_amount_per_min, *item_name)
            } else {
                eprintln!("{ERROR}no last input{RESET}");
                continue;
            }
        } else {
            let Some((amount, item_name)) = line.split_once(' ') else {
                eprintln!("{ERROR}invalid input{RESET}");
                continue;
            };
            let item_name = item_name.trim_start();
            let mut target_amount_per_min = parse_rational(amount);

            let Some((item_name, item)) = classes.items.iter().find(|(_, item)| {
                item.display_name.eq_ignore_ascii_case(item_name)
                    || item_name.strip_suffix("s").is_some_and(|without_suffix| {
                        item.display_name.eq_ignore_ascii_case(without_suffix)
                    })
            }) else {
                eprintln!("{ERROR}item {ITEM}{item_name} {ERROR}not found{RESET}");
                continue;
            };

            if item.stack_size == StackSize::Fluid {
                target_amount_per_min *= 1000;
            }

            last = Some((target_amount_per_min, item_name));

            (target_amount_per_min, item_name)
        };

        if target_amount_per_min.is_zero() {
            eprintln!("{ERROR}please enter a non-zero amount{RESET}");
            continue;
        }

        let find_by_ingredient = target_amount_per_min < Zero::zero();
        if find_by_ingredient {
            target_amount_per_min = -target_amount_per_min;
        }

        let recipes = classes.recipes.values().filter_map(|recipe| {
            if find_by_ingredient {
                &recipe.ingredients
            } else {
                &recipe.product
            }
            .0
            .iter()
            .find(|(current_item, _)| current_item == item_name)
            .map(|(_, amount)| (recipe, recipe.manufactoring_duration.recip() * amount))
        });

        let mut any = false;
        for (recipe, product_amount_per_sec) in recipes {
            any = true;

            let parallelism = target_amount_per_min / 60 / product_amount_per_sec;
            let machines = parallelism.ceil();
            let overclocking = parallelism / machines;

            let Some(machine) = recipe
                .produced_in
                .0
                .iter()
                .find_map(|machine| classes.machine.get(machine))
                .map(|machine| machine.display_name.as_str())
            else {
                // we don't care about recipes that don't have a machine
                continue;
            };

            println!("{HIGHLIGHT}┌── {RECIPE}{}", recipe.display_name);
            print!("{HIGHLIGHT}│ {RESET}🏭 {QUANTITY}{machines} {MACHINE}{machine}(s){RESET}");
            if !overclocking.is_one() {
                let overclock_percent = overclocking.to_f32().unwrap() * 100.0;
                print!(" at {OVERCLOCKING}{overclock_percent}{RESET}%",);
                print!(" ({OVERCLOCKING}{overclocking}{RESET})");
            }
            println!();

            for (item_amounts, arrow_color, arrow) in [
                (&recipe.ingredients, MACHINE_IN, '▶'),
                (&recipe.product, MACHINE_OUT, '◀'),
            ] {
                machine_io(
                    item_amounts,
                    &classes,
                    parallelism,
                    recipe.manufactoring_duration,
                    arrow_color,
                    arrow,
                    machines,
                    machine,
                );
            }

            println!("{HIGHLIGHT}└─{RESET}");
        }

        if !any {
            eprintln!("{ERROR}no recipe found{RESET}");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn machine_io(
    item_amounts: &ItemAmounts,
    classes: &Classes,
    parallelism: Rational64,
    manufactoring_duration: Rational64,
    arrow_color: &str,
    arrow: char,
    machines: Rational64,
    machine: &str,
) {
    for (item, amount) in &item_amounts.0 {
        let item = classes.items.get(item).unwrap();
        let mut throughput = parallelism * manufactoring_duration.recip() * amount * 60;
        if item.stack_size == StackSize::Fluid {
            throughput /= 1000;
        }
        print!(
            "{HIGHLIGHT}│  {arrow_color}{arrow} {THROUGHPUT}{:>9}{}/min {ITEM}{}{RESET}",
            throughput.to_f32().unwrap(),
            unit(item),
            item.display_name,
        );
        if machines != One::one() {
            print!(
                " ({THROUGHPUT}{}{}/min{RESET} per {MACHINE}{machine}{RESET})",
                (throughput / machines).to_f32().unwrap(),
                unit(item),
            );
        }
        println!();
    }
}

fn unit(item: &Item) -> &str {
    if item.stack_size == StackSize::Fluid {
        " m³"
    } else {
        ""
    }
}
