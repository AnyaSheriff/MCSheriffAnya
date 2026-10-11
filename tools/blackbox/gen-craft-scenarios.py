#!/usr/bin/env python3
# Генератор сценариев сверки крафта (craft-wood-types, craft-bed-wool, …): правка здесь, затем
#   python3 - < tools/blackbox/gen-craft-scenarios.py
# Номера ячеек: окно игрока — 0 итог, 1..4 сетка, 9..35 рюкзак, 36..44 панель; окно верстака —
# 0 итог, 1..9 сетка, 10..36 рюкзак, 37..45 панель.
import json, os

HERE = os.path.join(os.getcwd(), 'tools', 'blackbox')  # запускать из корня копии
OUT = os.path.join(HERE, 'scenarios')
PRE = ["clear $(bot)", "tp $(bot) $(0.5,0,2.5) 180 0", "gamemode survival $(bot)", "!stopfly"]
CLEAN = ["fill $(-5,0,-5) $(5,5,5) minecraft:air", "kill @e[type=item,distance=..48]", "clear $(bot)", "gamemode creative $(bot)"]
TABLE = {"at": [0, 0, 0], "state": "minecraft:crafting_table"}
OPEN = {"click": [0, 0, 0], "face": 1}
CLOSE_INV = {"close": True, "inventory": True}
CLOSE = {"close": True}


def wc(slot, button=0, mode=0, inv=False, bot=None):
    w = {"slot": slot, "button": button, "mode": mode}
    if inv:
        w["inventory"] = True
    a = {"window_click": w}
    if bot:
        a["bot"] = bot
    return a


def hot(n, item, count):
    return f"item replace entity $(bot) hotbar.{n} with minecraft:{item} {count}"


def write(name, about, setup, steps, cleanup=None, table=False, record=None, pre=PRE, gap=4, **extra):
    acts, t = [], 0
    for s in steps:
        if isinstance(s, int):
            t += s
            continue
        a = dict(s)
        a["after"] = t
        acts.append(a)
        t += gap
    d = {"name": name, "about": about + " [unverified: record against vanilla 26.1.2]", "blocks": [TABLE] if table else [],
         "setup": pre + setup, "actions": acts, "cleanup": cleanup or CLEAN, "record_ticks": record or t + 8,
         "origin": [0, -59, 0], "inventory": True, "item_components": True, "track_self": True, "track_mobs": True}
    d.update(extra)
    with open(os.path.join(OUT, name + '.json'), 'w') as f:
        json.dump(d, f, indent=2, ensure_ascii=False)
        f.write('\n')


# 1. Разные деревья в сетке 2×2: палки, смесь досок, верстак из вишни.
def stick_round(src):
    return [wc(36 + src, inv=True), wc(1, 1, inv=True), wc(3, 1, inv=True), wc(36 + src, inv=True), wc(0, inv=True), wc(9, inv=True)]


steps = []
for s in (0, 1, 2, 3):
    steps += stick_round(s)
steps += [wc(40, inv=True), wc(1, 1, inv=True), wc(41, inv=True), wc(3, 1, inv=True), wc(0, inv=True), wc(10, inv=True)]
steps += [wc(42, inv=True)] + [wc(i, 1, inv=True) for i in (1, 2, 3, 4)] + [wc(0, inv=True), wc(11, inv=True), CLOSE_INV]
write("craft-wood-types", "Sticks from birch, cherry and bamboo planks and from bamboo, sticks from mixed oak+mangrove planks, crafting table from cherry planks, all in the 2x2 grid.",
      [hot(0, "birch_planks", 4), hot(1, "cherry_planks", 4), hot(2, "bamboo_planks", 4), hot(3, "bamboo", 4), hot(4, "oak_planks", 1), hot(5, "mangrove_planks", 1), hot(6, "cherry_planks", 4)], steps)

# 2. Верстак: сундук из смеси досок (кольцо 3×3) и табурет... только сундук и бамбуковая мозаика не нужны.
ring = [1, 2, 3, 4, 6, 7, 8, 9]
steps = [OPEN, 4]
for slot, hotbar in zip(ring, (0, 0, 0, 1, 1, 1, 2, 2)):
    steps += [wc(37 + hotbar), wc(slot, 1), wc(37 + hotbar)]
steps += [wc(0), wc(10), CLOSE]
write("craft-table-chest-wood", "Chest from a ring of mixed birch, cherry and bamboo planks in the crafting table.",
      [hot(0, "birch_planks", 3), hot(1, "cherry_planks", 3), hot(2, "bamboo_planks", 2)], steps, table=True)

# 3. Кровать: три шерсти одного цвета + три доски; смесь цветов кровати не даёт.
steps = [OPEN, 4, wc(37), wc(1, 1), wc(2, 1), wc(3, 1), wc(38), wc(4, 1), wc(5, 1), wc(6, 1), wc(0), wc(10)]
steps += [wc(39), wc(1, 1), wc(2, 1), wc(40), wc(3, 1), wc(41), wc(4, 1), wc(5, 1), wc(6, 1), wc(0), CLOSE]
write("craft-bed-wool", "Lime bed from 3 lime wool + 3 oak planks; white+white+red wool with planks gives nothing.",
      [hot(0, "lime_wool", 3), hot(1, "oak_planks", 3), hot(2, "white_wool", 2), hot(3, "red_wool", 1), hot(4, "birch_planks", 3)], steps, table=True)

# 4. Shift-клик по итогу при полном инвентаре: 27 стопок булыжника в рюкзаке, панель 0..7 — булыжник,
#    в панели 8 брёвна; ячейки 36..44 заняты, места для досок нет.
fill = [f"item replace entity $(bot) inventory.{i} with minecraft:cobblestone 64" for i in range(27)]
fill += [hot(i, "cobblestone", 64) for i in range(8)] + [hot(8, "oak_log", 3)]
steps = [wc(44, inv=True), wc(1, 1, inv=True), wc(44, inv=True), wc(0, 0, 1, inv=True), wc(0, 0, 1, inv=True), CLOSE_INV]
write("craft-shift-full", "Shift-click on the planks result with a completely full inventory: nothing is moved and nothing is consumed.", fill, steps)

# 5. Доски частично помещаются: в ячейке 43 лежит 62 доски, больше места нет.
fill = [f"item replace entity $(bot) inventory.{i} with minecraft:cobblestone 64" for i in range(27)]
fill += [hot(i, "cobblestone", 64) for i in range(7)] + [hot(7, "oak_planks", 62), hot(8, "oak_log", 3)]
steps = [wc(44, inv=True), wc(1, 1, inv=True), wc(44, inv=True), wc(0, 0, 1, inv=True), wc(0, 0, 1, inv=True), CLOSE_INV]
write("craft-shift-partial", "Shift-click on the planks result when only 2 of 4 planks fit into the inventory (62+2).", fill, steps)

# 6. Два игрока у одного верстака: сетки независимы.
steps = [OPEN, {"click": [0, 0, 0], "face": 1, "bot": 2}, 4,
         wc(37), wc(1, 0), wc(2, 1), wc(37),
         wc(37, bot=2), wc(5, 0, bot=2), wc(37, bot=2),
         wc(0), wc(0, bot=2), CLOSE, {"close": True, "bot": 2}]
write("craft-two-players", "Two players with the same crafting table open: each has a private grid and result.",
      [hot(0, "oak_planks", 4), "tp $(bot2) $(1.5,0,2.5) 180 0", "gamemode survival $(bot2)", "clear $(bot2)",
       "item replace entity $(bot2) hotbar.0 with minecraft:birch_log 4"], steps, table=True,
      cleanup=CLEAN + ["clear $(bot2)", "gamemode creative $(bot2)", "tp $(bot2) $(6.5,0,6.5)"],
      second=True, track_second_window=True)

# 7. Верстак сломали, пока окно открыто: окно закрывается, предметы возвращаются.
steps = [OPEN, 4, wc(37), wc(1, 1), wc(2, 1), 4, {"set": {"at": [0, 0, 0], "state": "minecraft:air"}}, 6, wc(0), wc(37), CLOSE]
write("craft-table-break", "The crafting table is removed while open with items in the grid: the window is closed by the server and the grid items return.",
      [hot(0, "oak_log", 4)], steps, table=True)

# 8. Рецепты открываются при получении ингредиентов (книга рецептов): give, подбор предмета с земли.
steps = [{"cmd": "give $(bot) minecraft:oak_log 1"}, 6,
         {"cmd": "summon item $(0.5,0,2.5) {Item:{id:\"minecraft:iron_ingot\",count:1},PickupDelay:0s}"}, 6,
         {"cmd": "give $(bot) minecraft:cherry_planks 1"}, 6,
         {"cmd": "give $(bot) minecraft:bamboo 1"}, 6,
         {"cmd": "give $(bot) minecraft:cobblestone 1"}, 6,
         {"cmd": "give $(bot) minecraft:string 4"}, 6,
         {"cmd": "give $(bot) minecraft:white_wool 3"}]
write("craft-recipe-unlock", "Recipes added to the recipe book as ingredients are picked up (log, iron ingot from the ground, planks, bamboo, cobblestone, string, wool).",
      ["recipe take $(bot) *"], steps, track_recipes=True, gap=6)

# 9. limited_crafting: без открытого рецепта итог не появляется; открытый — появляется (рецепт доски
#    из брёвен открывает наличие бревна в рюкзаке, поэтому бревно одно и лежит в сетке, не в рюкзаке).
steps = [wc(36, inv=True), wc(1, 0, inv=True), 4, {"cmd": "recipe give $(bot) minecraft:oak_planks"}, 6,
         wc(1, 0, inv=True), wc(1, 0, inv=True), wc(0, inv=True), wc(9, inv=True), CLOSE_INV]
write("craft-limited", "limited_crafting=true: the planks result is withheld while the recipe is locked and appears after recipe give.",
      [hot(0, "oak_log", 1), "!wait 4", "recipe take $(bot) *", "gamerule limited_crafting true"], steps,
      cleanup=CLEAN + ["gamerule limited_crafting false", "recipe give $(bot) *"], track_recipes=True)

# 10. Все виды щелчков по сетке 2×2: сколько раз сервер шлёт итог и какие ячейки.
steps = [wc(36, inv=True),                      # взять 8 брёвен
         wc(1, inv=True),                       # положить все в сетку
         wc(1, inv=True),                       # забрать всю стопку из сетки
         wc(2, inv=True),                       # положить
         wc(2, 1, inv=True),                    # правой: взять половину
         wc(3, 1, inv=True),                    # правой: положить одно
         wc(3, 0, inv=True),                    # левой: долить остаток курсора
         wc(2, 0, 1, inv=True),                 # shift: из сетки в рюкзак
         wc(3, 0, 2, inv=True),                 # число 1 (панель 0): из сетки в панель
         wc(37, inv=True), wc(1, inv=True),     # доски в сетку
         wc(1, 0, 4, inv=True),                 # Q по ячейке сетки
         wc(1, 1, 4, inv=True),                 # Ctrl+Q по ячейке сетки
         wc(38, inv=True),                      # взять брёвна с панели
         wc(-999, 0, 5, inv=True), wc(1, 1, 5, inv=True), wc(2, 1, 5, inv=True), wc(4, 1, 5, inv=True), wc(-999, 2, 5, inv=True),  # раскладка по трём ячейкам
         wc(1, 0, 6, inv=True),                 # двойной щелчок
         wc(36, 0, 3, inv=True),                # [проверить] средняя кнопка (творческий) в выживании — ничего
         CLOSE_INV]
write("craft-grid-clicks", "Every click kind on the 2x2 grid (pickup, half, merge, shift, number key, Q, drag, double click): how many times the result slot is re-sent.",
      [hot(0, "oak_log", 8), hot(1, "oak_planks", 5), hot(2, "oak_log", 6)], steps)
