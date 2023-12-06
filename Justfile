# Create a new blog post file named `{year}-{month}-{day}-{fileName}.md`.
newBlogPost fileName:
    #!/bin/sh
    date="$(date +%F)"
    name="${date}-{{fileName}}.md"
    touch ./site/content/blog/${name}