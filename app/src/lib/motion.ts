// Длительность анимаций. Только короткие, только прозрачность и сдвиг
// (раздел 2 брифа и ТЗ: бережём слабые ПК). Если в Windows включено
// «Показывать анимацию в Windows: выкл.», анимаций нет совсем.

const reduce = window.matchMedia("(prefers-reduced-motion: reduce)");

export function ms(duration: number): number {
  return reduce.matches ? 0 : duration;
}
