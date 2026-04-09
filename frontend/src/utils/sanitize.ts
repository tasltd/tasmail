import DOMPurify from 'dompurify';

// Configure DOMPurify for email HTML rendering
const purifyConfig = {
  ALLOWED_TAGS: [
    'a', 'b', 'br', 'blockquote', 'code', 'div', 'em', 'h1', 'h2', 'h3',
    'h4', 'h5', 'h6', 'hr', 'i', 'img', 'li', 'ol', 'p', 'pre', 'span',
    'strong', 'table', 'tbody', 'td', 'th', 'thead', 'tr', 'u', 'ul', 'font',
    'center', 'sub', 'sup', 'small', 'big', 'strike', 's', 'del', 'ins',
    'abbr', 'address', 'article', 'aside', 'caption', 'cite', 'dd', 'dl', 'dt',
    'figcaption', 'figure', 'footer', 'header', 'main', 'mark', 'nav', 'section',
    'summary', 'time', 'wbr',
  ],
  ALLOWED_ATTR: [
    'href', 'src', 'alt', 'title', 'style', 'class', 'id', 'width', 'height',
    'align', 'valign', 'bgcolor', 'color', 'border', 'cellpadding', 'cellspacing',
    'colspan', 'rowspan', 'dir', 'lang', 'target', 'rel',
  ],
  ALLOW_DATA_ATTR: false,
  ADD_ATTR: ['target'],
  RETURN_DOM_FRAGMENT: false,
  RETURN_DOM: false,
};

export function sanitizeHtml(html: string): string {
  const clean = DOMPurify.sanitize(html, { ...purifyConfig, RETURN_TRUSTED_TYPE: false });
  return clean as string;
}

// Hook to add target="_blank" to all links after sanitization
DOMPurify.addHook('afterSanitizeAttributes', (node) => {
  if (node.tagName === 'A') {
    node.setAttribute('target', '_blank');
    node.setAttribute('rel', 'noopener noreferrer');
  }
});
