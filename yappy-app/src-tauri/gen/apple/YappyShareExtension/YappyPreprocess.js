// El preprocesado de Safari: este código corre DENTRO de la página que el
// usuario está viendo y entrega su DOM vivo a la extensión. Es la vía con
// sesión: si estás suscrito al periódico, el HTML llega SIN muro, tal cual
// lo veías. (Solo Safari invoca esto; desde otras apps llega la URL pelada
// y la app descarga como siempre.)
var YappyPreprocess = function () {};

YappyPreprocess.prototype = {
  run: function (args) {
    var html = "";
    try {
      html = document.documentElement ? document.documentElement.outerHTML : "";
    } catch (e) {
      html = "";
    }
    // Páginas descomunales: por encima de ~6 MB no es un artículo, y el
    // App Group no es un almacén; se recorta y Readability hará su parte.
    if (html.length > 6 * 1024 * 1024) {
      html = html.slice(0, 6 * 1024 * 1024);
    }
    args.completionFunction({
      url: document.baseURI || String(window.location.href || ""),
      titulo: String(document.title || ""),
      html: html,
    });
  },

  finalize: function () {},
};

var ExtensionPreprocessingJS = new YappyPreprocess();
