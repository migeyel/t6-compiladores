# autocraft.rs - T6 Construção de Compiladores 2022/1
Autores:
- Miguel Antonio de Oliveira — 772180
- Matheus Ramos de Carvalho — 769703

## Descrição do Problema
Vamos dizer que você esteja jogando Minecraft e queira craftar um item a partir de um inventário. O sistema de crafting em Minecraft é relativamente simples: São necessários até 9 itens diferentes em uma receita, que consome os itens e gera uma ou mais cópias de um único item. Algumas modificações feitas pela comunidade do jogo implementam computadores que realizam esse processo automaticamente, você disponibiliza um inventário para o computador junto com um pedido de um item a ser feito e o computador descobre quais receitas precisam ser aplicadas sobre o inventário para chegar no item alvo.

Curiosamente, essas implementações são ou extremamente lentas ou acabam retornando resultados incorretos, dizendo que alguns itens não são craftáveis enquanto na realidade existe uma sequência de receitas que resultam no item. Tomando inspiração nesse fato, vamos estudar a complexidade computacional desse problema, popularmente conhecido como o problema do autocrafting. Em 2018, Jonathan "SquidDev" Coates fez uma demonstração informal de que o problema é NP-difícil [1].

A redução de Coates é relativamente trivial, cada variável $x_i$ da fórmula 3-CNF-SAT de $n$ cláusulas é determinada por apenas um item $v_i$, junto com um conjunto de uma (ou mais) receitas $v_i \to 3n\cdot T_i$ e $v_i \to 3n \cdot F_i$, representando a atribuição de um valor à variável. Com os itens representando atribuição, podemos, por exemplo, formar a disjunção $x_i \lor \lnot x_j \lor x_k$ com três receitas: $T_i \to C_m$, $F_j \to C_m$ e $T_k \to C_m$, onde $m$ é o índice da cláusula. Finalmente, todas as cláusulas são juntadas em uma (ou mais) receitas da fórmula $C_0 + C_1 + \dots + C_{n-1} \to G$ e o item $G$ é definido como alvo.

Essa redução apresentada define um limite inferior para o problema, porém não é satisfatória. Gostaríamos, idealmente, de saber exatamente em qual classe de complexidade o problema de autocrafting está. Para isso, vamos explorar uma equivalência com o problema da alcançabilidade para uma generalização de redes de Petri.

Redes de Petri, descritas inicialmente por Carl Adam Petri em 1962 [2], são uma ferramenta útil para modelagem de sistemas distribuídos. Uma rede de Petri é composta por um grafo direcionado bipartido entre lugares $P_i$ e transições $T_i$. Cada lugar contém uma quantidade inteira de tokens. Cada transição se diz habilitada se existe pelo menos um token em todos os lugares que são ligados à transição por um arco. Uma transição habilitada pode disparar, subtraindo um token de todos os lugares de entrada e somando um a todos os lugares de saída. Por fim, o estado que descreve quantos tokens estão em cada lugar é denominado uma marcação da rede de Petri. O problema da alcançabilidade é simplesmente descobrir se uma marcação é alcançável disparando transições a partir de uma marcação inicial

Para nosso objetivo, vamos usar uma generalização dessas redes, onde o grafo também é ponderado. As condições e efeitos do disparo se estendem trivialmente para esse tipo de grafo. É fácil ver que esse modelo é equivalente ao problema do autocrafting, cada item possível é representado por um lugar, cada receita é representada por uma transição e o inventário inicial é uma marcação inicial da rede.

Equipados com esse modelo, vemos que, como toda receita tem no máximo um item específico de saída, nossa rede cai na classificação descrita por [3], onde é demonstrado que o problema da alcançabilidade é PSPACE-completo. Porém, a equivalência não é exata. Para isso, precisamos de uma transição extra que toma como entrada a marcação do estado de aceitação da máquina PSPACE e toma como saída um item novo, que será o item alvo.

Para pesquisa futura, damos atenção a alguns casos específicos no jogo onde mais de um item é usado como saída. Por exemplo, para fazer um bolo, é necessário um balde de leite, que é retornado vazio no fim da receita. Esperamos que esse evento seja suficiente para colocar o problema na classe EXPSPACE-difícil, porém deixamos esse resultado em aberto.

## Implementação
Usamos o dedutor automático Z3 [4] para resolver um subconjunto de instâncias. Em
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
2. Petri, Carl Adam. "Kommunikation mit automaten." (1962).
3. Mayr, Ernst W., and Jeremias Weihmann. "Completeness results for generalized communication-free Petri nets with arbitrary edge multiplicities." International Workshop on Reachability Problems. Springer, Berlin, Heidelberg, 2013.
4. https://github.com/Z3Prover/z3
