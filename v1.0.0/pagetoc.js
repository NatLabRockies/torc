// Page Table of Contents - builds a right sidebar TOC from page headings
(function() {
    'use strict';

    function buildPageToc() {
        // Get the main content area
        const content = document.querySelector('.content main');
        if (!content) return;

        // Find all h2 and h3 headings with IDs
        const headings = content.querySelectorAll('h2[id], h3[id]');
        if (headings.length === 0) return;

        // Create the TOC container
        const tocContainer = document.createElement('nav');
        tocContainer.className = 'pagetoc';
        tocContainer.setAttribute('aria-label', 'Page table of contents');

        const tocTitle = document.createElement('div');
        tocTitle.className = 'pagetoc-title';
        tocTitle.textContent = 'On this page';
        tocContainer.appendChild(tocTitle);

        const tocList = document.createElement('ul');
        tocList.className = 'pagetoc-list';

        headings.forEach(function(heading) {
            const li = document.createElement('li');
            li.className = 'pagetoc-item';
            if (heading.tagName === 'H3') {
                li.className += ' pagetoc-item-h3';
            }

            const link = document.createElement('a');
            link.href = '#' + heading.id;
            link.textContent = heading.textContent.replace(/^#\s*/, '').replace(/\s*#$/, '');
            link.className = 'pagetoc-link';

            li.appendChild(link);
            tocList.appendChild(li);
        });

        tocContainer.appendChild(tocList);

        // Insert the TOC into the page
        const contentWrap = document.querySelector('.content');
        if (contentWrap) {
            contentWrap.appendChild(tocContainer);
        }

        // Highlight active section on scroll
        function updateActiveLink() {
            const scrollPos = window.scrollY + 100;
            let activeLink = null;

            headings.forEach(function(heading) {
                if (heading.offsetTop <= scrollPos) {
                    activeLink = heading.id;
                }
            });

            tocList.querySelectorAll('.pagetoc-link').forEach(function(link) {
                if (link.getAttribute('href') === '#' + activeLink) {
                    link.classList.add('active');
                } else {
                    link.classList.remove('active');
                }
            });
        }

        window.addEventListener('scroll', updateActiveLink, { passive: true });
        updateActiveLink();
    }

    // Run after DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', buildPageToc);
    } else {
        buildPageToc();
    }
})();
