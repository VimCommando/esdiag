// HTMX upload service event handlers
document.addEventListener("htmx:beforeRequest", function (evt) {
  if (evt.detail.target.id === "upload-service-result") {
    const serviceButton = document.getElementById("upload-service-button");
    serviceButton.disabled = true;
    serviceButton.value = "Submitting...";
  }
});

document.addEventListener("htmx:afterRequest", function (evt) {
  if (evt.detail.target.id === "upload-service-result") {
    const serviceButton = document.getElementById("upload-service-button");
    serviceButton.disabled = false;
    serviceButton.value = "Submit";

    // Clear form on successful request
    if (evt.detail.xhr.status >= 200 && evt.detail.xhr.status < 300) {
      document.getElementById("upload-service-form").reset();
      clearCurlForm();
    }
  }
});

document.addEventListener("htmx:responseError", function (evt) {
  if (evt.detail.target.id === "upload-service-result") {
    const serviceButton = document.getElementById("upload-service-button");
    serviceButton.disabled = false;
    serviceButton.value = "Submit";
  }
});
