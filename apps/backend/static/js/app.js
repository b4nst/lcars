/* LCARS HTMX Frontend - Custom JavaScript */

// Keyboard shortcuts
document.addEventListener('keydown', function(e) {
    // Escape to close modals
    if (e.key === 'Escape') {
        const modal = document.querySelector('.lcars-modal-backdrop');
        if (modal) {
            modal.remove();
        }
    }

    // / to focus search (when not in input)
    if (e.key === '/' && !['INPUT', 'TEXTAREA'].includes(document.activeElement.tagName)) {
        e.preventDefault();
        const searchInput = document.querySelector('.search-input');
        if (searchInput) {
            searchInput.focus();
        }
    }
});

// Close modal when clicking backdrop
document.addEventListener('click', function(e) {
    if (e.target.classList.contains('lcars-modal-backdrop')) {
        e.target.remove();
    }
});

// HTMX event handlers
document.addEventListener('htmx:afterSwap', function(e) {
    // Focus first input in newly loaded modals
    const modal = e.detail.target.querySelector('.lcars-modal');
    if (modal) {
        const input = modal.querySelector('input[type="text"]');
        if (input) {
            input.focus();
        }
    }
});

// Handle SSE connection errors
document.addEventListener('htmx:sseError', function(e) {
    console.warn('SSE connection error:', e.detail);
});

// Confirm before destructive actions
document.addEventListener('htmx:confirm', function(e) {
    if (e.detail.question) {
        if (!confirm(e.detail.question)) {
            e.preventDefault();
        }
    }
});
