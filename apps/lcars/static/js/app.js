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

// Custom LCARS confirmation modal
function showConfirmModal(message, onConfirm) {
    const backdrop = document.createElement('div');
    backdrop.className = 'lcars-modal-backdrop';

    // Build modal structure safely to prevent XSS
    const modal = document.createElement('div');
    modal.className = 'lcars-modal';
    modal.style.maxWidth = '30rem';

    const header = document.createElement('div');
    header.className = 'lcars-modal-header';
    header.style.background = 'var(--lcars-red)';
    const h2 = document.createElement('h2');
    h2.textContent = 'Confirm';
    header.appendChild(h2);

    const body = document.createElement('div');
    body.className = 'lcars-modal-body';
    const p = document.createElement('p');
    p.style.textTransform = 'none';
    p.textContent = message; // Safe: textContent escapes HTML
    body.appendChild(p);

    const footer = document.createElement('div');
    footer.className = 'lcars-modal-footer';
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'lcars-button yellow sm';
    cancelBtn.dataset.action = 'cancel';
    cancelBtn.textContent = 'Cancel';
    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'lcars-button red sm';
    confirmBtn.dataset.action = 'confirm';
    confirmBtn.textContent = 'Remove';
    footer.appendChild(cancelBtn);
    footer.appendChild(confirmBtn);

    modal.appendChild(header);
    modal.appendChild(body);
    modal.appendChild(footer);
    backdrop.appendChild(modal);

    // Close on backdrop click
    backdrop.addEventListener('click', function(e) {
        if (e.target === backdrop) {
            backdrop.remove();
        }
    });

    // Cancel button
    cancelBtn.addEventListener('click', function() {
        backdrop.remove();
    });

    // Confirm button
    confirmBtn.addEventListener('click', function() {
        backdrop.remove();
        onConfirm();
    });

    document.body.appendChild(backdrop);
    cancelBtn.focus();
}

// Confirm before destructive actions using custom modal
document.addEventListener('htmx:confirm', function(e) {
    // Only intercept if there's a confirmation question
    if (!e.detail.question) return;

    // Prevent the default request and built-in confirm
    e.preventDefault();

    // Capture issueRequest before showing modal (closure)
    var issueRequest = e.detail.issueRequest;

    showConfirmModal(e.detail.question, function() {
        // Pass true to skip the built-in window.confirm()
        issueRequest(true);
    });
});
