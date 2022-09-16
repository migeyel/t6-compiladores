# autocraft.rs - T6 Construção de Compiladores 2022/1
Autores:
- Miguel Antonio de Oliveira — 772180
- Matheus Ramos de Carvalho — 769703

## Descrição do Problema
Vamos dizer que você esteja jogando Minecraft e queira craftar um item a partir de um inventário. O sistema de crafting em Minecraft é relativamente simples: São necessários até 9 itens diferentes em uma receita, que consome os itens e gera uma ou mais cópias de um único item. Algumas modificações feitas pela comunidade do jogo implementam computadores que realizam esse processo automaticamente, você disponibiliza um inventário para o computador junto com um pedido de um item a ser feito e o computador descobre quais receitas precisam ser aplicadas sobre o inventário para chegar no item alvo.

Curiosamente, essas implementações são ou extremamente lentas ou acabam retornando resultados incorretos, dizendo que alguns itens não são craftáveis enquanto na realidade existe uma sequência de receitas que resultam no item. Tomando inspiração nesse fato, vamos estudar a complexidade computacional desse problema, popularmente conhecido como o problema do autocrafting. Em 2018, Jonathan "SquidDev" Coates fez uma demonstração informal de que o problema é NP-difícil [1].
## Implementação
Usamos o dedutor automático Z3 [2] para resolver um subconjunto de instâncias. Em
específico, proibimos instâncias com ciclos.

### Linguagem de descrição
Instâncias são descritas em uma linguagem de descrição seguindo o exemplo:
```
# Uma quantidade inicial no inventário, representada por um número e um nome.
2 log
3 cobblestone

# Uma receita, representada por uma equação de transição.
# - Duas receitas não podem ter o mesmo nome.
# - O mesmo item não pode aparecer duas vezes no mesmo lado da receita.
# - Receitas não podem formar ciclos (ver acima).
craft_planks: 1 log -> 4 plank
craft_sticks: 2 plank -> 4 stick
craft_stone_pickaxe: 3 cobblestone + 2 stick -> 1 stone_pickaxe

# Uma saída desejada.
out 1 stone_pickaxe
```
O sistema tenta achar uma sequência de receitas que leva o inventário a ter pelo
menos a quantidade desejada de saídas. A saída do programa no exemplo é:
```
The system is CRAFTABLE:
1 × craft_planks
2 × craft_sticks
1 × craft_stone_pickaxe
```

## Execução
1. `docker build -t t6 .`
2. `cat examples/basic.txt | docker run --rm -i t6`

## Referências
1. https://squiddev.cc/2018/01/26/ae-sat.html
2. https://github.com/Z3Prover/z3
