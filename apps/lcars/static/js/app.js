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
    backdrop.innerHTML = `
        <div class="lcars-modal" style="max-width: 30rem;">
            <div class="lcars-modal-header" style="background: var(--lcars-red);">
                <h2>Confirm</h2>
            </div>
            <div class="lcars-modal-body">
                <p style="text-transform: none;">${message}</p>
            </div>
            <div class="lcars-modal-footer">
                <button class="lcars-button yellow sm" data-action="cancel">Cancel</button>
                <button class="lcars-button red sm" data-action="confirm">Remove</button>
            </div>
        </div>
    `;

    const cancelBtn = backdrop.querySelector('[data-action="cancel"]');
    const confirmBtn = backdrop.querySelector('[data-action="confirm"]');

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
