// La llegada: el instante teatral en que lo compartido cae en Yappy y el
// loro se lo traga antes de empezar a hablar. shareIntake lo dispara con el
// título; el caparazón móvil pinta la escena por encima de todo y la
// desmonta sola (o al primer toque).
export const llegada = $state<{ titulo: string | null }>({ titulo: null });

export function soltarLlegada(titulo: string) {
  llegada.titulo = titulo;
  setTimeout(() => {
    if (llegada.titulo === titulo) llegada.titulo = null;
  }, 2100);
}
