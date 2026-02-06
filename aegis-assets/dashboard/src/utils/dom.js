export function createElement(tag, options = {}) {
  const element = document.createElement(tag);
  const { className, text, children, attributes } = options;

  if (className) {
    element.className = className;
  }

  if (text) {
    element.textContent = text;
  }

  if (attributes) {
    Object.entries(attributes).forEach(([key, value]) => {
      element.setAttribute(key, value);
    });
  }

  if (children) {
    children.forEach((child) => {
      if (child) {
        element.appendChild(child);
      }
    });
  }

  return element;
}

export function formatTimestamp(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "Pending";
  }
  return date.toLocaleString();
}
