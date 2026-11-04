# Набор обложек сборок по умолчанию (раздел 6.2 ТЗ, design/covers.md): 12 сюжетов
# на сетке 48×48 настоящих пикселей — интерфейс увеличивает их в целое число раз
# без сглаживания. Нарисованы кодом, без чужих текстур и персонажей; набор можно
# заменить своими картинками (scripts/process_covers.py приводит их к той же сетке).
#
#   python scripts/covers.py   → app/src/assets/covers/NN-name.png
import math
import os
import random

from PIL import Image

N = 48
OUT = os.path.join(os.path.dirname(__file__), '..', 'app', 'src', 'assets', 'covers')


def hexc(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


class Canvas:
    def __init__(self, seed):
        self.img = Image.new('RGB', (N, N))
        self.px = self.img.load()
        self.rnd = random.Random(seed)

    def put(self, x, y, c):
        if 0 <= x < N and 0 <= y < N:
            self.px[x, y] = hexc(c) if isinstance(c, str) else c

    def rect(self, x0, y0, x1, y1, c):
        for y in range(max(0, y0), min(N, y1 + 1)):
            for x in range(max(0, x0), min(N, x1 + 1)):
                self.put(x, y, c)

    def sky(self, bands):
        # Небо полосами сверху вниз: [(до_строки, цвет), ...] — без плавных градиентов.
        y = 0
        for until, c in bands:
            self.rect(0, y, N - 1, until, c)
            y = until + 1

    def disc(self, cx, cy, r, c):
        for y in range(cy - r, cy + r + 1):
            for x in range(cx - r, cx + r + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + r * 0.6:
                    self.put(x, y, c)

    def ridge(self, base, amp, freq, phase, c, top=None, rough=0.0):
        # Блочный рельеф: высота ступенями по 2 пикселя.
        heights = []
        for x in range(0, N, 2):
            h = base - amp * (0.5 + 0.5 * math.sin(x * freq + phase)) - self.rnd.random() * rough
            heights.append(int(h))
        for i, h in enumerate(heights):
            for dx in range(2):
                x = i * 2 + dx
                self.rect(x, h, x, N - 1, c)
                if top:
                    self.put(x, h, top)
        return heights

    def tree(self, x, ground, h, trunk, leaves, leaves_dark):
        self.rect(x, ground - h, x + 1, ground - 1, trunk)
        top = ground - h - 4
        self.rect(x - 2, top, x + 3, top + 4, leaves)
        self.rect(x - 1, top - 2, x + 2, top - 1, leaves)
        self.rect(x - 2, top + 3, x + 3, top + 4, leaves_dark)

    def pine(self, x, ground, h, trunk, leaves, snow=None):
        self.rect(x, ground - 2, x, ground - 1, trunk)
        for i in range(h):
            w = (h - i) // 2 + 1
            y = ground - 2 - i
            self.rect(x - w + 1, y, x + w - 1, y, leaves)
            if snow and i % 3 == 0:
                self.rect(x - w + 1, y, x - w + 2, y, snow)

    def save(self, name):
        os.makedirs(OUT, exist_ok=True)
        self.img.save(os.path.join(OUT, name + '.png'), optimize=True)


def forest():
    c = Canvas(1)
    c.sky([(5, '#f6c1a6'), (11, '#f8d6a2'), (17, '#fbe7b6'), (22, '#fff1cc')])
    c.disc(36, 15, 4, '#fff6d8')
    c.ridge(27, 4, 0.18, 1.0, '#7fae8a')
    g = c.ridge(33, 3, 0.25, 0.0, '#4f8a4a', top='#6fa85c')
    for x in (3, 11, 30, 41):
        c.tree(x, 33, 5, '#6b4a2b', '#3f7a3a', '#2f5e2c')
    for x in (20, 25):
        c.tree(x, 34, 6, '#e8e2d0', '#5f9b4b', '#467a38')
    c.rect(0, 38, N - 1, 42, '#4f9fc9')
    for x in range(0, N, 6):
        c.rect(x + 1, 40, x + 3, 40, '#8cc9e6')
    c.rect(0, 43, N - 1, 47, '#3d6f35')
    return c, '01-forest'


def mountains():
    c = Canvas(2)
    c.sky([(10, '#5b9bd5'), (20, '#7db3e3'), (28, '#a7cdf0')])
    for x, y in ((6, 6), (32, 9)):
        c.rect(x, y, x + 5, y + 1, '#ffffff')
        c.rect(x + 1, y - 1, x + 3, y - 1, '#ffffff')
    peaks = [(14, 12, 13), (32, 8, 15), (44, 16, 10)]
    for px, py, w in peaks:
        for y in range(py, 40):
            half = (y - py) * w // 24 + 1
            for x in range(px - half, px + half + 1):
                shade = '#8d96a6' if x < px else '#6f7889'
                c.put(x, y, shade)
            if y < py + 5:
                c.rect(px - half, y, px + half, y, '#f4f7fb')
    c.ridge(38, 3, 0.2, 2.0, '#5d9b4f', top='#78b562')
    for x in (4, 9, 37, 43):
        c.pine(x, 41, 8, '#5a3d24', '#2f6b3a')
    c.rect(0, 42, N - 1, 47, '#4a8640')
    return c, '02-mountains'


def cave():
    c = Canvas(3)
    c.rect(0, 0, N - 1, N - 1, '#1d2433')
    for y in range(N):
        for x in range(N):
            if c.rnd.random() < 0.15:
                c.put(x, y, '#252e40')
    # Свод и сталактиты.
    for x in range(N):
        h = int(5 + 3 * math.sin(x * 0.4) + c.rnd.random() * 2)
        c.rect(x, 0, x, h, '#3a3f4d')
    for x, l in ((7, 7), (18, 5), (29, 9), (41, 6)):
        for i in range(l):
            w = max(0, (l - i) // 3)
            c.rect(x - w, 6 + i, x + w, 6 + i, '#3a3f4d')
    # Пол, лава и её отсвет.
    c.rect(0, 38, N - 1, N - 1, '#3a3f4d')
    c.rect(0, 37, N - 1, 37, '#4a5060')
    c.rect(12, 36, 35, 37, '#6a4636')
    c.rect(15, 39, 32, 43, '#e2571e')
    c.rect(17, 40, 30, 41, '#ffab3d')
    c.rect(13, 38, 14, 39, '#7a4a32')
    c.rect(33, 38, 34, 39, '#7a4a32')
    # Сталагмиты и руда в стенах.
    for x, h in ((6, 8), (40, 10)):
        for i in range(h):
            w = (h - i) // 4
            c.rect(x - w, 37 - i, x + w, 37 - i, '#4a5060')
    for ox, oy, col in ((3, 20, '#5fd0e0'), (44, 25, '#5fd0e0'), (37, 16, '#e0b84a'), (10, 28, '#e0b84a')):
        c.rect(ox, oy, ox + 1, oy + 1, col)
    # Факелы с тёплым отсветом на стене.
    for tx, ty in ((11, 19), (36, 22)):
        c.rect(tx - 2, ty - 4, tx + 2, ty + 1, '#2f2c2e')
        c.rect(tx, ty, tx, ty + 4, '#7a5a35')
        c.rect(tx - 1, ty - 2, tx + 1, ty - 1, '#ffb347')
        c.put(tx, ty - 3, '#ffe08a')
    return c, '03-cave'


def inferno():
    c = Canvas(4)
    c.sky([(8, '#3a0d10'), (16, '#5c1416'), (26, '#7d1d18'), (32, '#9a2a1a')])
    c.ridge(30, 5, 0.22, 0.3, '#5a1f1c', top='#7a2b24')
    for x in (10, 34):
        c.rect(x, 8, x + 2, 34, '#ff8a1f')
        c.rect(x + 1, 8, x + 1, 34, '#ffd04a')
    c.rect(0, 36, N - 1, 47, '#e2571e')
    for x in range(0, N, 5):
        c.rect(x, 38 + (x % 3), x + 2, 38 + (x % 3), '#ffab3d')
    for fx, fy in ((22, 28), (40, 29), (5, 31)):
        c.rect(fx, fy, fx, fy + 6, '#6e2a3a')
        c.rect(fx - 3, fy - 3, fx + 3, fy - 1, '#c2335a')
        c.rect(fx - 2, fy - 4, fx + 2, fy - 4, '#e0527a')
    return c, '04-inferno'


def void_islands():
    c = Canvas(5)
    c.rect(0, 0, N - 1, N - 1, '#140d24')
    for _ in range(40):
        c.put(c.rnd.randrange(N), c.rnd.randrange(N), c.rnd.choice(['#6f5fa8', '#b9aee6', '#3a2d63']))
    for ix, iy, w in ((6, 34, 16), (28, 28, 14), (16, 42, 10)):
        c.rect(ix, iy, ix + w, iy + 2, '#e9e3a8')
        c.rect(ix + 2, iy + 3, ix + w - 2, iy + 4, '#c9c27e')
        c.rect(ix + 5, iy + 5, ix + w - 5, iy + 5, '#a69e5c')
    for px, top in ((11, 20), (33, 14)):
        c.rect(px, top, px + 3, top + 14, '#1b1530')
        c.rect(px + 1, top, px + 1, top + 14, '#2a2147')
        c.rect(px + 1, top - 2, px + 2, top - 1, '#c58cff')
        c.put(px + 1, top - 3, '#efd6ff')
    return c, '05-void-islands'


def ocean():
    c = Canvas(6)
    c.sky([(12, '#62b6e8'), (22, '#86c9ef'), (28, '#b2def5')])
    c.disc(9, 9, 4, '#fff3b0')
    c.rect(0, 29, N - 1, N - 1, '#1f9bb5')
    for y in range(31, N, 3):
        for x in range((y * 7) % 6, N, 9):
            c.rect(x, y, x + 2, y, '#5fd0e0')
    c.rect(20, 30, 39, 32, '#f2d98a')
    c.rect(23, 28, 36, 29, '#f2d98a')
    c.rect(29, 15, 30, 27, '#8a5a32')
    c.rect(24, 13, 35, 14, '#3c9a4a')
    c.rect(22, 15, 25, 16, '#3c9a4a')
    c.rect(34, 15, 37, 16, '#3c9a4a')
    c.rect(27, 12, 32, 12, '#57b25c')
    return c, '06-ocean'


def winter():
    c = Canvas(7)
    c.sky([(10, '#1f2b55'), (18, '#2f3f72'), (26, '#4a5a8f')])
    for _ in range(30):
        c.put(c.rnd.randrange(N), c.rnd.randrange(30), '#d6e2ff')
    c.ridge(32, 3, 0.2, 1.0, '#c9d6ee', top='#eef3ff')
    for x in (4, 12, 38, 45):
        c.pine(x, 36, 10, '#4a3420', '#26493f', snow='#eef3ff')
    c.rect(20, 26, 31, 35, '#7a5530')
    c.rect(19, 23, 32, 25, '#eef3ff')
    c.rect(21, 21, 30, 22, '#eef3ff')
    c.rect(23, 29, 25, 31, '#ffd36b')
    c.rect(28, 29, 29, 34, '#4e3520')
    c.rect(0, 36, N - 1, N - 1, '#dfe8fa')
    for x in range(0, N, 7):
        c.put(x + 2, 40, '#c2cfe8')
    return c, '07-winter'


def desert():
    c = Canvas(8)
    c.sky([(8, '#f08a3e'), (16, '#f6a54e'), (24, '#fbc46d'), (28, '#fde09b')])
    c.disc(36, 20, 5, '#fff0c2')
    c.ridge(34, 6, 0.15, 2.5, '#e8b66a', top='#f2c983')
    c.rect(14, 22, 31, 34, '#d9a35c')
    c.rect(16, 18, 29, 21, '#d9a35c')
    c.rect(19, 15, 26, 17, '#d9a35c')
    c.rect(20, 28, 25, 34, '#7a4f25')
    c.rect(14, 22, 31, 22, '#c48d47')
    c.ridge(41, 3, 0.25, 0.0, '#e8b66a', top='#f6d391')
    for x, h in ((6, 7), (40, 9)):
        c.rect(x, 41 - h, x + 1, 40, '#3f8a45')
        c.rect(x - 2, 41 - h + 2, x - 2, 41 - h + 4, '#3f8a45')
        c.rect(x + 3, 41 - h + 3, x + 3, 41 - h + 5, '#3f8a45')
    c.rect(0, 44, N - 1, N - 1, '#f2c983')
    return c, '08-desert'


def village():
    c = Canvas(9)
    c.sky([(12, '#79bde9'), (24, '#a5d6f5')])
    c.ridge(30, 3, 0.2, 0.5, '#6aaa52', top='#86c463')
    for hx, hy in ((5, 22), (31, 23)):
        c.rect(hx, hy + 4, hx + 11, hy + 10, '#b07a45')
        c.rect(hx, hy + 9, hx + 11, hy + 10, '#8a8a8a')
        c.rect(hx - 1, hy + 2, hx + 12, hy + 3, '#7a3e2b')
        c.rect(hx + 1, hy, hx + 10, hy + 1, '#7a3e2b')
        c.rect(hx + 3, hy + 6, hx + 4, hy + 7, '#ffe08a')
        c.rect(hx + 8, hy + 6, hx + 9, hy + 8, '#5a3a20')
    c.rect(21, 14, 25, 29, '#d8cfb6')
    for i in range(-5, 6):
        c.put(23 + i, 14 + abs(i) // 2 - 3, '#9c6b3e')
        c.put(23, 14 + i, '#9c6b3e')
    c.rect(0, 34, N - 1, 47, '#d9c25a')
    for y in range(35, N, 2):
        for x in range(y % 3, N, 3):
            c.put(x, y, '#c2a947')
    for y in range(33, N):
        w = (y - 33) // 2 + 2
        c.rect(23 - w, y, 24 + w, y, '#a88a5c')
    return c, '09-village'


def tech():
    c = Canvas(10)
    c.rect(0, 0, N - 1, N - 1, '#2a2f38')
    for y in range(0, N, 8):
        c.rect(0, y, N - 1, y, '#232830')
    c.rect(0, 36, N - 1, N - 1, '#3b414c')
    for x in range(0, N, 4):
        c.rect(x, 39, x + 1, 39, '#59606d')
    for mx, my, w, h in ((4, 18, 12, 17), (30, 14, 14, 21)):
        c.rect(mx, my, mx + w, my + h, '#5c6573')
        c.rect(mx + 1, my + 1, mx + w - 1, my + 1, '#7a8494')
        c.rect(mx + 2, my + 4, mx + w - 2, my + 6, '#1b2230')
        c.rect(mx + 3, my + 5, mx + w // 2, my + 5, '#36e0e0')
        c.rect(mx + 3, my + 9, mx + 5, my + 11, '#ff9a3d')
        c.rect(mx + w - 5, my + 9, mx + w - 3, my + 11, '#36e0e0')
    c.rect(16, 24, 30, 26, '#8a93a3')
    c.rect(16, 25, 30, 25, '#a7b0bf')
    c.rect(10, 4, 11, 17, '#8a93a3')
    c.rect(10, 4, 37, 5, '#8a93a3')
    c.rect(36, 4, 37, 13, '#8a93a3')
    c.rect(0, 33, N - 1, 34, '#4a515d')
    for x in range(2, N, 6):
        c.rect(x, 33, x + 2, 33, '#ffcf4a')
    return c, '10-tech'


def magic():
    c = Canvas(11)
    c.rect(0, 0, N - 1, N - 1, '#1e1530')
    for sx in (0, 36):
        c.rect(sx, 6, sx + 11, 40, '#5a3b22')
        for y in range(8, 40, 6):
            for x in range(sx + 1, sx + 11, 2):
                c.rect(x, y, x, y + 3, c.rnd.choice(['#8c2f39', '#2f5d8c', '#3d7a3a', '#c9a24a', '#6b3d8c']))
            c.rect(sx, y + 4, sx + 11, y + 4, '#4a2f1b')
    c.rect(0, 40, N - 1, N - 1, '#3a2a4a')
    c.rect(16, 30, 31, 39, '#2b2335')
    c.rect(15, 28, 32, 29, '#8a2f3a')
    c.rect(20, 25, 27, 27, '#e0d6c0')
    c.rect(23, 25, 24, 27, '#b9a77f')
    for _ in range(26):
        x = 23 + int(c.rnd.gauss(0, 7))
        y = 18 + int(c.rnd.gauss(0, 6))
        c.put(x, y, c.rnd.choice(['#b77bff', '#e2c2ff', '#8a5cff']))
    return c, '11-magic'


def castle():
    c = Canvas(12)
    c.sky([(8, '#3b2a6b'), (16, '#7a3e7a'), (24, '#c45a5a'), (30, '#f08a4e')])
    c.disc(40, 10, 3, '#ffe0c2')
    c.ridge(38, 6, 0.12, 4.0, '#2b2436', top='#3a3148')
    c.rect(12, 18, 35, 37, '#4a4558')
    for x in range(12, 36, 3):
        c.rect(x, 16, x + 1, 17, '#4a4558')
    for tx in (8, 34):
        c.rect(tx, 10, tx + 5, 37, '#3d394a')
        for x in range(tx, tx + 6, 2):
            c.put(x, 9, '#3d394a')
        c.rect(tx + 2, 14, tx + 3, 15, '#ffb347')
    c.rect(20, 27, 27, 37, '#1a1424')
    c.rect(21, 28, 26, 37, '#9b4dff')
    c.rect(22, 29, 25, 36, '#c58cff')
    for wx in (15, 31):
        c.rect(wx, 22, wx + 1, 23, '#ffb347')
    c.rect(0, 40, N - 1, N - 1, '#221c2c')
    return c, '12-castle'


if __name__ == '__main__':
    for make in (forest, mountains, cave, inferno, void_islands, ocean, winter, desert, village, tech, magic, castle):
        canvas, name = make()
        canvas.save(name)
        print(name)
