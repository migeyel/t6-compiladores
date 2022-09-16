use std::{collections::{HashMap, hash_map::Entry, HashSet}, io::Read};

use anyhow::{anyhow, bail};
use pathfinding::directed::topological_sort::topological_sort;
use pest::{Parser as _, iterators::Pair};
use z3::{Context, Config};

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
struct T6Parser;

#[derive(Debug)]
struct System {
    /// Nomes dos itens, indexados por id.
    item_names: Vec<String>,
    /// Id dos itens, indexados por nome.
    item_ids: HashMap<String, usize>,
    /// Inventário
    inventory: Vec<u64>,
    /// Pedidos de saída.
    requests: Vec<u64>,
    /// Nomes das receitas, indexados por id.
    recipe_names: Vec<String>,
    /// Id das receitas, indexados por nome.
    recipe_ids: HashMap<String, usize>,
    /// Conteúdo das receitas, indexados por id.
    recipes: Vec<(HashMap<usize, u64>, HashMap<usize, u64>)>,
    /// Vetor de id de receitas ordenadas topologicamente.
    sorted_recipe_ids: Vec<usize>,
}

impl System {
    /// Retorna o id do item dado seu nome, criadno um novo caso não exista.
    fn get_item_id(&mut self, name: &str) -> usize {
        if let Some(&id) = self.item_ids.get(name) {
            id
        } else {
            let id = self.item_names.len();
            self.item_names.push(name.to_string());
            self.item_ids.insert(name.to_string(), id);
            self.inventory.push(0);
            self.requests.push(0);
            id
        }
    }

    /// Anda na AST de um item, obtendo seu id e quantidade.
    fn walk_item2(&mut self, pair: Pair<Rule>) -> (usize, u64) {
        // Desestruturar os pares:
        // item = { natural ~ ident }
        let mut inner = pair.into_inner();
        let amt: u64 = inner.next().unwrap().as_str().parse().unwrap();
        let item_name = inner.next().unwrap().as_str();
        let item_id = self.get_item_id(item_name);
        (item_id, amt)
    }

    /// Anda na AST de uma declaração de inventário, adicionando-a ao estado.
    /// # Errors
    /// Um erro é retornado se outra declaração do mesmo item já existe.
    fn walk_item(&mut self, pair: Pair<Rule>) -> anyhow::Result<()> {
        let (item_id, amt) = self.walk_item2(pair);

        // Verificar se a declaração já existe.
        if self.inventory[item_id] != 0 {
            let item_name = &self.item_names[item_id];
            Err(anyhow!("item {item_name} declared twice"))
        } else {
            self.inventory[item_id] = amt;
            Ok(())
        }
    }

    /// Anda na AST de um pedido de item, adicionando-o ao estado.
    /// # Errors
    /// Um erro é retornado se outro pedido do mesmo item já existe.
    fn walk_request(&mut self, pair: Pair<Rule>) -> anyhow::Result<()> {
        // Desestruturar os pares:
        // request = { "out" ~ item }
        let item = pair.into_inner().next().unwrap();
        let (item_id, amt) = self.walk_item2(item);

        // Verificar se o pedido já existe.
        if self.requests[item_id] != 0 {
            let item_name = &self.item_names[item_id];
            Err(anyhow!("request {item_name} declared twice"))
        } else {
            self.requests[item_id] = amt;
            Ok(())
        }
    }

    /// Anda na AST de uma lista de itens, retornando um mapa.
    /// # Errors
    /// Um erro é retornado se um item aparece mais de uma vez na lista.
    fn walk_set(&mut self, pair: Pair<Rule>) -> anyhow::Result<HashMap<usize, u64>> {
        let mut out = HashMap::new();

        // Desestruturar os pares:
        // item_set = { item ~ ("+" ~ item)* }
        for item in pair.into_inner() {
            let (ident, amt) = self.walk_item2(item);

            // Verificar se o item já existe na lista.
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

    /// Anda na AST de uma receita, adicionando-a ao estado.
    /// # Errors
    /// Um erro é retornado se um item aparece mais de uma vez na lista de
    /// entrada ou saída, ou se outra receita com o mesmo nome já existe.
    fn walk_recipe(&mut self, pair: Pair<Rule>) -> anyhow::Result<()> {
        // Desestruturar os pares:
        // recipe = { ident ~ ":" ~ item_set ~ "->" ~ item_set }
        let mut inner = pair.into_inner();
        let name = inner.next().unwrap().as_str();
        let inputs = self.walk_set(inner.next().unwrap())?;
        let outputs = self.walk_set(inner.next().unwrap())?;

        // Verificar se a receita já existe.
        if self.recipe_ids.get(name).is_some() {
            bail!("recipe {name} declared twice");
        }

        // Inserir no estado.
        let recipe_id = self.recipes.len();
        self.recipe_ids.insert(name.to_string(), recipe_id);
        self.recipe_names.push(name.to_string());
        self.recipes.push((inputs, outputs));

        Ok(())
    }

    /// Transforma código fonte em um novo estado.
    fn parse(input: &str) -> anyhow::Result<Self> {
        // Inicializar
        let mut out = Self {
            item_names: Vec::new(),
            item_ids: HashMap::new(),
            inventory: Vec::new(),
            requests: Vec::new(),
            recipe_names: Vec::new(),
            recipe_ids: HashMap::new(),
            recipes: Vec::new(),
            sorted_recipe_ids: Vec::new(),
        };

        // Desestruturar os pares:
        // set = { SOI ~ (item | recipe | request)* ~ EOI }
        let set = T6Parser::parse(Rule::set, input)?.next().unwrap();
        for pair in set.into_inner() {
            // Chamar funções depdendendo de qual regra foi encontrada.
            match pair.as_rule() {
                Rule::item => out.walk_item(pair)?,
                Rule::request => out.walk_request(pair)?,
                Rule::recipe => out.walk_recipe(pair)?,
                Rule::EOI => break,
                _ => unreachable!(),
            }
        }

        out.sort_recipe_ids()?;
        
        Ok(out)
    }

    // Ordena topologicamente as receitas do estado.
    // # Errors
    // Um erro é retornado caso o sistema de receitas seja cíclico.
    fn sort_recipe_ids(&mut self) -> anyhow::Result<()> {
        // Constrói a lista de arestas do sistema.
        let mut item_edges = vec![HashSet::new(); self.item_names.len()];
        let mut recipe_edges = vec![HashSet::new(); self.recipe_names.len()];
        for (recipe_id, (inputs, outputs)) in self.recipes.iter().enumerate() {
            for (&input_id, _) in inputs.iter() {
                item_edges[input_id].insert(recipe_id);
            }

            for (&output_id, _) in outputs.iter() {
                recipe_edges[recipe_id].insert(output_id);
            }
        }

        // Constrói os nós do sistema.
        let mut roots = Vec::new();
        roots.extend((0..item_edges.len()).map(Node::Item));
        roots.extend((0..recipe_edges.len()).map(Node::Recipe));

        // Função de cálculo de sucessores do grafo.
        let successors = |node: &Node| -> Box<dyn Iterator<Item = Node>> {
            match node {
                Node::Item(id) => Box::new(item_edges[*id].iter().map(|&i| Node::Recipe(i))),
                Node::Recipe(id) => Box::new(recipe_edges[*id].iter().map(|&i| Node::Item(i))),
            }
        };

        // Ordenação e retorno do erro.
        let sort = match topological_sort(&roots, successors) {
            Ok(s) => s,
            Err(n) => match n {
                Node::Item(i) => {
                    let name = &self.item_names[i];
                    bail!("item \"{name}\" forms a cycle in the system");
                }

                Node::Recipe(i) => {
                    let name = &self.recipe_names[i];
                    bail!("recipe \"{name}\" forms a cycle in the system");
                }
            },
        };

        // Transforma os nós em ids.
        self.sorted_recipe_ids = sort.iter()
            .filter_map(|n| match n {
                Node::Item(_) => None,
                Node::Recipe(id) => Some(*id),
            })
            .collect();

        Ok(())
    }

    /// Resolve um estado.
    fn solve(&self) -> Craftability {
        // Construir os deltas de inventario (inventario - pedidos).
        let inventory_delta = self.inventory.iter()
            .zip(self.requests.iter())
            .map(|(&i, &r)| i as i64 - r as i64)
            .collect::<Vec<_>>();

        // Construir os deltas de receita (saídas - entradas).
        let mut recipe_deltas = Vec::new();
        for (inputs, outputs) in self.recipes.iter() {
            let mut delta = HashMap::new();
            for (&item_id, &amt) in inputs.iter() {
                *delta.entry(item_id).or_default() -= amt as i64;
            }
            for (&item_id, &amt) in outputs.iter() {
                *delta.entry(item_id).or_default() += amt as i64;
            }
            recipe_deltas.push(delta);
        }
        
        // Preparar o solver.
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = z3::Solver::new(&ctx);
        let zero = z3::ast::Int::from_u64(&ctx, 0);

        // Preparar o vetor de incidência.
        // Restrição extra: incidência não pode ser negativa.
        let mut recipe_incidence = Vec::new();
        for i in 0..recipe_deltas.len() {
            let name = self.recipe_names[i].as_str();
            recipe_incidence.push(z3::ast::Int::new_const(&ctx, name));
            solver.assert(&recipe_incidence[i].ge(&zero));
        }

        // Transformar os deltas de receitas em expressões partindo da incidência.
        let mut exprs = vec![vec![]; inventory_delta.len()];
        for (recipe_id, recipe_delta) in recipe_deltas.iter().enumerate() {
            for (&item_id, &amt) in recipe_delta.iter() {
                let coeff = z3::ast::Int::from_i64(&ctx, amt);
                let slice = &[&coeff, &recipe_incidence[recipe_id]];
                let expr = z3::ast::Int::mul(&ctx, slice);
                exprs[item_id].push(expr);
            }
        }
        
        // Colocar os deltas do inventário no modelo.
        for (i, &amt) in inventory_delta.iter().enumerate() {
            exprs[i].push(z3::ast::Int::from_i64(&ctx, amt));
        }

        // Transformar as expressões em booleanos pela relação ≥ 0.
        let inventory = exprs.iter()
            .map(|vec| z3::ast::Int::add(&ctx, &vec.iter().collect::<Vec<_>>()))
            .map(|int| int.ge(&zero))
            .collect::<Vec<_>>();

        // Juntar os booleanos por conjunção.
        let model = inventory.iter()
            .collect::<Vec<_>>();

        // Colocar a conjunção global no modelo.
        let model = z3::ast::Bool::and(&ctx, &model);

        // Resolver
        solver.assert(&model);

        // Retornar falhas cedo.
        match solver.check() {
            z3::SatResult::Unsat => return Craftability::Uncraftable,
            z3::SatResult::Unknown => return Craftability::Unknown,
            z3::SatResult::Sat => {}
        }

        // Obter as incidências do modelo de volta do Z3, em ordem topológica.
        let model = solver.get_model().unwrap();
        let mut inv2 = self.inventory.clone();
        let mut solution = Vec::new();
        for &recipe_id in self.sorted_recipe_ids.iter() {
            let (inputs, outputs) = &self.recipes[recipe_id];

            let incidence = model.eval(&recipe_incidence[recipe_id], false)
                .unwrap()
                .as_u64()
                .unwrap();

            if incidence == 0 { continue; }

            for (&input_id, &amt) in inputs {
                let amt = amt * incidence;
                if inv2[input_id] < amt { return Craftability::Unknown; }
                inv2[input_id] -= amt;
            }

            for (&output_id, &amt) in outputs {
                let amt = amt * incidence;
                inv2[output_id] += amt;
            }

            solution.push((self.recipe_names[recipe_id].to_string(), incidence));
        }

        Craftability::Craftable(solution)
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum Node {
    Item(usize),
    Recipe(usize),
}

/// A decisão do programa sobre um sistema.
#[derive(Debug)]
enum Craftability {
    /// É possível chegar na saída aplicando as receitas dadas em ordem.
    Craftable(Vec<(String, u64)>),
    /// Não é possível chegar na saída aplicando receitas.
    Uncraftable,
    /// O Z3 não sabe a resposta do sistema.
    Unknown,
}

fn main() -> anyhow::Result<()> {
    // Ler o código fonte da entrada padrão.
    let mut src = Vec::new();
    std::io::stdin().lock().read_to_end(&mut src)?;
    let src = String::from_utf8(src)?;

    // Parse e resolução.
    let system = System::parse(&src)?;
    let res = system.solve();

    // Reportar resultado.
    match res {
        Craftability::Uncraftable => println!("The system is UNCRAFTABLE"),
        Craftability::Unknown => bail!("the solver gave up"),
        Craftability::Craftable(rs) => {
            println!("The system is CRAFTABLE:");
            for (r, amt) in rs.iter() {
                println!("{} × {}", amt, r);
            }
        }
    }

    Ok(())
}
