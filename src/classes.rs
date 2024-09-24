mod raw;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::read,
    marker::PhantomData,
    path::Path,
    str::FromStr,
};

use derive_where::derive_where;
use encoding_rs::UTF_16LE;
use num_rational::Rational64;
use raw::{Class, ClassCollection};
use serde::Deserialize;

#[derive(Debug)]
pub struct Classes {
    pub items: BTreeMap<ClassName<Item>, Item>,
    pub recipes: BTreeMap<ClassName<Recipe>, Recipe>,
    pub machine: BTreeMap<ClassName<Machine>, Machine>,
}

impl Classes {
    pub fn load(path: &Path) -> Self {
        let bytes = read(path).unwrap();
        let (text, malformed) = UTF_16LE.decode_with_bom_removal(&bytes);
        if malformed {
            println!("malformed");
        }

        let class_collections: Vec<ClassCollection> = serde_json::from_str(&text).unwrap();

        class_collections.into_iter().fold(
            Self {
                items: Default::default(),
                recipes: Default::default(),
                machine: Default::default(),
            },
            |mut acc, class_collection| {
                match class_collection
                    .native_class
                    .strip_prefix("/Script/CoreUObject.Class'/Script/FactoryGame.")
                    .unwrap()
                    .strip_suffix('\'')
                    .unwrap()
                {
                    "FGItemDescriptor"
                    | "FGItemDescriptorBiomass"
                    | "FGItemDescriptorNuclearFuel"
                    | "FGItemDescriptorPowerBoosterFuel"
                    | "FGResourceDescriptor"
                    | "FGBuildingDescriptor"
                    | "FGPowerShardDescriptor"
                    | "FGVehicleDescriptor"
                    | "FGAmmoTypeProjectile"
                    | "FGConsumableDescriptor"
                    | "FGAmmoTypeSpreadshot"
                    | "FGEquipmentDescriptor"
                    | "FGPoleDescriptor"
                    | "FGAmmoTypeInstantHit" => {
                        acc.items
                            .extend(class_collection.classes.into_iter().map(|class| {
                                (
                                    ClassName::new(class.name.to_owned()),
                                    Item::from_class(class),
                                )
                            }))
                    }
                    "FGRecipe" => {
                        acc.recipes
                            .extend(class_collection.classes.into_iter().map(|class| {
                                (
                                    ClassName::new(class.name.to_owned()),
                                    Recipe::from_class(class),
                                )
                            }))
                    }
                    "FGBuildableManufacturer" | "FGBuildableManufacturerVariablePower" => acc
                        .machine
                        .extend(class_collection.classes.into_iter().map(|class| {
                            (
                                ClassName::new(class.name.to_owned()),
                                Machine::from_class(class),
                            )
                        })),
                    _ => {}
                }

                acc
            },
        )
    }
}

#[derive_where(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassName<T>(String, PhantomData<fn(T) -> T>);

impl<T> std::fmt::Debug for ClassName<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<T> ClassName<T> {
    pub fn new(name: String) -> Self {
        Self(name, PhantomData)
    }
}

#[derive(Debug)]
pub struct Machine {
    pub display_name: String,
}

trait FromClass: Sized {
    fn from_class(class: Class) -> Self;
}

impl FromClass for Machine {
    fn from_class(class: Class) -> Self {
        Self {
            display_name: class.get_string("mDisplayName").to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct Item {
    pub display_name: String,
    pub stack_size: StackSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackSize {
    One,
    Small,
    Medium,
    Big,
    Huge,
    Fluid,
}

impl Item {
    fn from_class(class: Class) -> Self {
        Self {
            display_name: class.get_string("mDisplayName").to_owned(),
            stack_size: match class.get_string("mStackSize") {
                "SS_ONE" => StackSize::One,
                "SS_SMALL" => StackSize::Small,
                "SS_MEDIUM" => StackSize::Medium,
                "SS_BIG" => StackSize::Big,
                "SS_HUGE" => StackSize::Huge,
                "SS_FLUID" => StackSize::Fluid,
                s => panic!("unknown stack size: {s}"),
            },
        }
    }
}

#[derive(Debug)]
pub struct Recipe {
    pub display_name: String,
    pub produced_in: ClassSet<Machine>,
    pub ingredients: ItemAmounts,
    pub product: ItemAmounts,
    pub manufactoring_duration: Rational64,
}

impl Recipe {
    fn from_class(class: Class) -> Self {
        Self {
            display_name: class.get_string("mDisplayName").parse().unwrap(),
            produced_in: class.get_string("mProducedIn").parse().unwrap(),
            ingredients: class.get_string("mIngredients").parse().unwrap(),
            product: class.get_string("mProduct").parse().unwrap(),
            manufactoring_duration: parse_rational(class.get_string("mManufactoringDuration")),
        }
    }
}

pub fn parse_rational(text: &str) -> Rational64 {
    let Some((whole, fract)) = text.split_once('.') else {
        return text.parse().unwrap();
    };
    let full = whole.to_string() + fract;
    Rational64::new(full.parse().unwrap(), 10i64.pow(fract.len() as u32))
}

pub struct ItemAmounts(pub Vec<(ClassName<Item>, i64)>);

impl std::fmt::Debug for ItemAmounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ItemAmounts {
    type Err = ();

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Self(Default::default()))
        } else {
            s = s.strip_prefix('(').unwrap();
            s = s.strip_suffix(')').unwrap();
            let s = s
                .replace('(', "{")
                .replace(')', "}")
                .replace("ItemClass=", "\"item_class\":")
                .replace("Amount=", "\"amount\":");
            let json_array = format!("[{s}]");
            let stacks: Vec<ItemStack> = serde_json::from_str(&json_array).unwrap();
            Ok(Self(
                stacks
                    .into_iter()
                    .map(|item_stack| {
                        (
                            {
                                let long_name: &str = &item_stack.item_class;
                                ClassName::new(
                                    long_name
                                        .rsplit_once('.')
                                        .unwrap()
                                        .1
                                        .strip_suffix('\'')
                                        .unwrap()
                                        .to_owned(),
                                )
                            },
                            item_stack.amount,
                        )
                    })
                    .collect(),
            ))
        }
    }
}

#[derive_where(Default)]
pub struct ClassSet<T>(pub BTreeSet<ClassName<T>>);

impl<T> std::fmt::Debug for ClassSet<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> FromStr for ClassSet<T> {
    type Err = ();

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Default::default())
        } else {
            s = s.strip_prefix('(').unwrap();
            s = s.strip_suffix(')').unwrap();
            let json_array = format!("[{}]", s);
            let names: Vec<String> = serde_json::from_str(&json_array).unwrap();
            Ok(Self(
                names
                    .into_iter()
                    .map(|s| {
                        let long_name: &str = &s;
                        ClassName::new(long_name.rsplit_once('.').unwrap().1.to_owned())
                    })
                    .collect(),
            ))
        }
    }
}

#[derive(Deserialize)]
struct ItemStack {
    item_class: String,
    amount: i64,
}
