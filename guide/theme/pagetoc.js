function forEach(elems, fun) {
  Array.prototype.forEach.call(elems, fun);
}

function getPagetoc(){
  const pagetoc = document.getElementsByClassName("pagetoc")[0];

  if (pagetoc) {
    return pagetoc;
  }

  return autoCreatePagetoc();
}

function autoCreatePagetoc() {
  const main = document.querySelector("#content > main");

  const content = document.createElement("div");
  content.classList.add("content-wrap");
  content.append(...main.childNodes);

  main.appendChild(content);
  main.classList.add("wrapped");

  main.insertAdjacentHTML("beforeend", `
    <div class="sidetoc">
      <nav class="pagetoc"></nav>
    </div>
  `);

  return document.getElementsByClassName("pagetoc")[0]
}

function getPagetocElems() {
  return getPagetoc().children;
}

function getHeaders(){
  return document.getElementsByClassName("header")
}

// Un-active everything when you click it
function forPagetocElem(fun) {
  forEach(getPagetocElems(), fun);
}

function getRect(element) {
  return element.getBoundingClientRect();
}

function overflowTop(container, element) {
  return getRect(container).top - getRect(element).top;
}

function overflowBottom(container, element) {
  return getRect(container).bottom - getRect(element).bottom;
}

var activeHref = location.href;

var updateFunction = function (elem = undefined) {
  var id = elem;

  if (!id && location.href != activeHref) {
    activeHref = location.href;
    forPagetocElem(function (el) {
      if (el.href === activeHref) {
        id = el;
      }
    });
  }

  if (!id) {
    var elements = getHeaders();
    let margin = window.innerHeight / 3;

    forEach(elements, function (el, i, arr) {
      if (!id && getRect(el).top >= 0) {
        if (getRect(el).top < margin) {
          id = el;
        } else {
          id = arr[Math.max(0, i - 1)];
        }
      }
      // a very long last section
      // its heading is over the screen
      if (!id && i == arr.length - 1) {
        id = el
      }
    });
  }

  forPagetocElem(function (el) {
    el.classList.remove("active");
  });

  if (!id) return;

  forPagetocElem(function (el) {
    if (id.href.localeCompare(el.href) == 0) {
      el.classList.add("active");
      let pagetoc = getPagetoc();
      if (overflowTop(pagetoc, el) > 0) {
        pagetoc.scrollTop = el.offsetTop;
      }
      if (overflowBottom(pagetoc, el) < 0) {
        pagetoc.scrollTop -= overflowBottom(pagetoc, el);
      }
    }
  });
};

let elements = getHeaders();

if (elements.length > 1) {
  // Populate sidebar on load
  window.addEventListener("load", function () {
    var pagetoc = getPagetoc();
    var elements = getHeaders();
    forEach(elements, function (el) {
      var link = document.createElement("a");
      link.appendChild(document.createTextNode(el.text));
      link.href = el.hash;
      link.classList.add("pagetoc-" + el.parentElement.tagName);
      pagetoc.appendChild(link);
      link.onclick = function () {
        updateFunction(link);
      };
    });
    updateFunction();
  });

  // Handle active elements on scroll
  window.addEventListener("scroll", function () {
    updateFunction();
  });
} else {
  getPagetoc();
  const sidetoc = document.getElementsByClassName("sidetoc");
  if (sidetoc.length > 0 ) {
    document.getElementsByClassName("sidetoc")[0].remove();
  }
}
