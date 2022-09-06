use std::{fs, collections::{HashMap, hash_map::Entry}};

use anyhow::{anyhow, bail};
use pest::{Parser as _, iterators::Pair};
use clap::Parser as _;
use z3::{Context, Config};


#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
struct T6Parser;

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(value_parser)]
    path: String,
}

#[derive(Debug)]
struct System {
    inventory: HashMap<String, u64>,
    requests: HashMap<String, u64>,
    recipes: HashMap<String, (HashMap<String, u64>, HashMap<String, u64>)>,
}

impl System {
    fn walk_item2(pair: Pair<Rule>) -> anyhow::Result<(String, u64)> {
        let mut inner = pair.into_inner();
        let p1 = inner.next().unwrap();
        match p1.as_rule() {
            Rule::ident => {
                Ok((p1.as_str().to_string(), 1))
            }

            Rule::natural => {
                let amt: u64 = p1.as_str().parse().unwrap();
                let name = inner.next().unwrap().as_str().to_string();
                Ok((name, amt))
            }

            _ => unreachable!(),
        }
    }

    fn walk_item(&mut self, pair: Pair<Rule>) -> anyhow::Result<()> {
        let (ident, amt) = Self::walk_item2(pair)?;
        match self.inventory.entry(ident) {
            Entry::Occupied(entry) => {
                let item = entry.key();
                Err(anyhow!("item {item} declared twice"))
            }

            Entry::Vacant(entry) => {
                entry.insert(amt);
                Ok(())
            }
        }
    }

    fn walk_request(&mut self, pair: Pair<Rule>) -> anyhow::Result<()> {
        let pair = pair.into_inner().next().unwrap();
        let (ident, amt) = Self::walk_item2(pair)?;
        match self.requests.entry(ident) {
            Entry::Occupied(entry) => {
                let item = entry.key();
                Err(anyhow!("request {item} declared twice"))
            }

            Entry::Vacant(entry) => {
                entry.insert(amt);
                Ok(())
            }
        }
    }

    fn walk_set(pair: Pair<Rule>) -> anyhow::Result<HashMap<String, u64>> {
        let mut out = HashMap::new();
        for item in pair.into_inner() {
            let (ident, amt) = Self::walk_item2(item)?;
            match out.entry(ident) {
                Entry::Occupied(entry) => {
                    let item = entry.key();
                    bail!("item {item} repeated twice in recipe");
                }

                Entry::Vacant(entry) => {
                    entry.insert(amt);
                }
            }
        }
        Ok(out)
    }

    fn walk_recipe(&mut self, pair: Pair<Rule>) -> anyhow::Result<()> {
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap().as_str();
        let inputs = Self::walk_set(inner.next().unwrap())?;
        let outputs = Self::walk_set(inner.next().unwrap())?;
        match self.recipes.entry(name.to_string()) {
            Entry::Occupied(entry) => {
                let recipe = entry.key();
                bail!("recipe {recipe} declared twice");
            }

            Entry::Vacant(entry) => {
                entry.insert((inputs, outputs));
                Ok(())
            }
        }
    }

    fn parse(input: &str) -> anyhow::Result<Self> {
        let mut out = Self {
            inventory: HashMap::new(),
            requests: HashMap::new(),
            recipes: HashMap::new()
        };

        let set = T6Parser::parse(Rule::set, input)?.next().unwrap();
        for pair in set.into_inner() {
            match pair.as_rule() {
                Rule::item => out.walk_item(pair)?,
                Rule::request => out.walk_request(pair)?,
                Rule::recipe => out.walk_recipe(pair)?,
                Rule::EOI => return Ok(out),
                _ => unreachable!(),
            }
        }
        
        unreachable!()
    }

    fn complete(&mut self) {
        for (_, (input, output)) in self.recipes.iter() {
            for (item, _) in input.iter() {
                if let None = self.inventory.get(item) {
                    self.inventory.insert(item.to_string(), 0);
                }
            }

            for (item, _) in output.iter() {
                if let None = self.inventory.get(item) {
                    self.inventory.insert(item.to_string(), 0);
                }
            }
        }

        for (item, _) in self.requests.iter() {
            if let None = self.inventory.get(item) {
                self.inventory.insert(item.to_string(), 0);
            }
        }
    }

    fn to_state_equation(&self) -> StateEquation {
        let mut item_ids = HashMap::new();
        let mut inventory = Vec::new();
        for (item, amt) in self.inventory.iter() {
            item_ids.insert(item.clone(), item_ids.len());
            inventory.push(*amt as i64);
        }

        for (item, amt) in self.requests.iter() {
            inventory[item_ids[item]] -= *amt as i64;
        }
        
        let mut recipe_names = Vec::new();
        let mut recipes = Vec::new();
        for (name, (inputs, outputs)) in self.recipes.iter() {
            let mut value: HashMap<usize, i64> = HashMap::new();

            for (item, amt) in inputs.iter() {
                let iid = item_ids[item];
                *value.entry(iid).or_default() += *amt as i64;
            }

            for (item, amt) in outputs.iter() {
                let iid = item_ids[item];
                *value.entry(iid).or_default() -= *amt as i64;
            }

            recipe_names.push(name.clone());
            recipes.push(value);
        }

        StateEquation { item_ids, recipe_names, inventory, recipes }
    }
}

#[derive(Debug)]
struct StateEquation {
    item_ids: HashMap<String, usize>,
    recipe_names: Vec<String>,
    inventory: Vec<i64>,
    recipes: Vec<HashMap<usize, i64>>,
}

enum Resultao {
    Sim,
    Não,
    Talvez,
}

impl StateEquation {
    fn instance(&self) -> Resultao {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = z3::Solver::new(&ctx);
        let zero = z3::ast::Int::from_u64(&ctx, 0);

        let mut recipe_incidence = Vec::new();
        for i in 0..self.recipes.len() {
            let name = format!("recipe {i}");
            recipe_incidence.push(z3::ast::Int::new_const(&ctx, name));
            solver.assert(&recipe_incidence[i].ge(&zero));
        }

        let mut exprs = vec![vec![]; self.inventory.len()];
        for (recipe_index, recipe) in self.recipes.iter().enumerate() {
            for (item_index, amt) in recipe.iter() {
                let coeff = z3::ast::Int::from_i64(&ctx, *amt);
                let slice = &[&coeff, &recipe_incidence[recipe_index]];
                let expr = z3::ast::Int::mul(&ctx, slice);
                exprs[*item_index].push(expr);
            }
        }

        for (i, amt) in self.inventory.iter().enumerate() {
            exprs[i].push(z3::ast::Int::from_i64(&ctx, *amt));
        }

        let inventory = exprs.iter()
            .map(|vec| z3::ast::Int::add(&ctx, &vec.iter().collect::<Vec<_>>()))
            .map(|int| int.ge(&zero))
            .collect::<Vec<_>>();

        let model = inventory.iter()
            .collect::<Vec<_>>();

        let model = z3::ast::Bool::and(&ctx, &model);

        solver.assert(&model);
        dbg!(solver.check());

        match solver.check() {
            z3::SatResult::Unsat => return Resultao::Não,
            z3::SatResult::Unknown => return Resultao::Talvez,
            z3::SatResult::Sat => {}
        }

        let model = solver.get_model().unwrap();

        for i in 0..self.recipes.len() {
            dbg!(&self.recipe_names[i]);
            dbg!(model.eval(&recipe_incidence[i], false));
        }

        todo!()
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let src = fs::read_to_string(args.path)?;
    let mut foo = System::parse(&src)?;
    foo.complete();
    foo.to_state_equation().instance();
    Ok(())
}
