// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="introduction.html">Introduction</a></li><li class="chapter-item expanded affix "><li class="spacer"></li><li class="chapter-item expanded "><a href="getting-started/index.html"><strong aria-hidden="true">1.</strong> Getting started</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="getting-started/pre-reqs.html"><strong aria-hidden="true">1.1.</strong> Pre-requisites</a></li><li class="chapter-item expanded "><a href="getting-started/installation.html"><strong aria-hidden="true">1.2.</strong> Installation</a></li><li class="chapter-item expanded "><a href="getting-started/project.html"><strong aria-hidden="true">1.3.</strong> First project</a></li></ol></li><li class="chapter-item expanded "><a href="commands/index.html"><strong aria-hidden="true">2.</strong> Commands</a></li><li class="chapter-item expanded "><a href="configuration/index.html"><strong aria-hidden="true">3.</strong> Configuration</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="configuration/schema.html"><strong aria-hidden="true">3.1.</strong> Schema</a></li></ol></li><li class="chapter-item expanded "><a href="build/hooks.html"><strong aria-hidden="true">4.</strong> Hooks</a></li><li class="chapter-item expanded "><a href="assets/index.html"><strong aria-hidden="true">5.</strong> Assets</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="assets/minification.html"><strong aria-hidden="true">5.1.</strong> Minification</a></li><li class="chapter-item expanded "><a href="assets/sri.html"><strong aria-hidden="true">5.2.</strong> Sub-resource integrity</a></li></ol></li><li class="chapter-item expanded "><a href="advanced/index.html"><strong aria-hidden="true">6.</strong> Advanced</a></li><li><ol class="section"><li class="chapter-item expanded "><a href="advanced/javascript_interop.html"><strong aria-hidden="true">6.1.</strong> JavaScript interoperability</a></li><li class="chapter-item expanded "><a href="advanced/startup_event.html"><strong aria-hidden="true">6.2.</strong> Startup event</a></li><li class="chapter-item expanded "><a href="advanced/initializer.html"><strong aria-hidden="true">6.3.</strong> Application initializer</a></li><li class="chapter-item expanded "><a href="advanced/library.html"><strong aria-hidden="true">6.4.</strong> Library crate</a></li><li class="chapter-item expanded "><a href="advanced/paths.html"><strong aria-hidden="true">6.5.</strong> Base URLs, public URLs, paths &amp; reverse proxies</a></li><li class="chapter-item expanded "><a href="advanced/proxy.html"><strong aria-hidden="true">6.6.</strong> Backend Proxy</a></li></ol></li><li class="chapter-item expanded "><li class="spacer"></li><li class="chapter-item expanded affix "><a href="contributing.html">Contributing</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString();
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
