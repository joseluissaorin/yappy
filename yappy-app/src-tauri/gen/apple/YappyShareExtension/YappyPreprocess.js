// El preprocesado de Safari: este código corre DENTRO de la página que el
// usuario está viendo y entrega su DOM vivo a la extensión. Es la vía con
// sesión: si estás suscrito al periódico, el HTML llega SIN muro, tal cual
// lo veías. (Solo Safari invoca esto; desde otras apps llega la URL pelada
// y la app descarga como siempre.)
//
// Un PDF abierto en Safari no tiene DOM que valga: la página es el visor
// (un <embed type="application/pdf">). Entonces se descarga el PDF desde la
// propia página, con la sesión del usuario (un PDF de PRADO o de un campus
// virtual pide estar dentro), y viaja a la extensión en base64.
var YappyPreprocess = function () {};

// Por encima de esto el PDF no cabe en la extensión (límite de ~120 MB de
// memoria, y el base64 lo infla un tercio): la app lo intentará por la URL.
var PDF_MAX = 30 * 1024 * 1024;

function esVisorDePdf() {
  if (document.contentType === "application/pdf") return true;
  var embed = document.querySelector('embed[type="application/pdf"]');
  return !!embed && document.body && document.body.children.length <= 3;
}

function base64DeBuffer(buf) {
  var bytes = new Uint8Array(buf);
  var trozos = [];
  var paso = 0x8000;
  for (var i = 0; i < bytes.length; i += paso) {
    trozos.push(String.fromCharCode.apply(null, bytes.subarray(i, i + paso)));
  }
  return btoa(trozos.join(""));
}

YappyPreprocess.prototype = {
  run: function (args) {
    var url = document.baseURI || String(window.location.href || "");
    var titulo = String(document.title || "");
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
    var entregar = function (pdf) {
      args.completionFunction({ url: url, titulo: titulo, html: html, pdf: pdf || "" });
    };
    if (!esVisorDePdf()) {
      entregar("");
      return;
    }
    var embed = document.querySelector('embed[type="application/pdf"]');
    var origen = (embed && embed.src) || String(window.location.href || "");
    url = origen;
    try {
      fetch(origen, { credentials: "include" })
        .then(function (r) {
          if (!r.ok) throw new Error("HTTP " + r.status);
          return r.arrayBuffer();
        })
        .then(function (buf) {
          var cab = new Uint8Array(buf.slice(0, 5));
          var esPdf = String.fromCharCode.apply(null, cab) === "%PDF-";
          entregar(esPdf && buf.byteLength <= PDF_MAX ? base64DeBuffer(buf) : "");
        })
        .catch(function () {
          entregar("");
        });
    } catch (e) {
      entregar("");
    }
  },

  finalize: function () {},
};

var ExtensionPreprocessingJS = new YappyPreprocess();
