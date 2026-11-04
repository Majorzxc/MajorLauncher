# Свои обложки по умолчанию (design/covers.md): картинки из генератора или
# нарисованные вручную приводятся к общему виду набора — сетка 48×48 настоящих
# пикселей и палитра из 24 цветов. Рядом кладётся копия 240×240 для просмотра.
#
#   python scripts/process_covers.py <папка с исходниками>
#   → app/src/assets/covers/<имя>.png (заменяют нарисованные кодом)
import os
import sys

from PIL import Image

N = 48
COLORS = 24
OUT = os.path.join(os.path.dirname(__file__), '..', 'app', 'src', 'assets', 'covers')


def process(src: str, dst: str) -> None:
    im = Image.open(src).convert('RGB')
    side = min(im.size)
    left, top = (im.width - side) // 2, (im.height - side) // 2
    im = im.crop((left, top, left + side, top + side))
    # Сначала усреднение (BOX), чтобы «пиксели» генератора разного размера слились ровно.
    small = im.resize((N, N), Image.BOX)
    small = small.quantize(colors=COLORS, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE).convert('RGB')
    small.save(dst, optimize=True)
    small.resize((N * 5, N * 5), Image.NEAREST).save(dst.replace('.png', '.preview.png'))


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit('python scripts/process_covers.py <папка с исходниками>')
    os.makedirs(OUT, exist_ok=True)
    for name in sorted(os.listdir(sys.argv[1])):
        if name.lower().endswith(('.png', '.jpg', '.jpeg', '.webp')):
            dst = os.path.join(OUT, os.path.splitext(name)[0] + '.png')
            process(os.path.join(sys.argv[1], name), dst)
            print(name, '→', os.path.relpath(dst))
    print('Превью *.preview.png не коммитить: удалите их после просмотра.')
