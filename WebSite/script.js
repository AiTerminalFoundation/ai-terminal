document.addEventListener('DOMContentLoaded', function () {
    // Set current year in footer
    document.getElementById('current-year').textContent = new Date().getFullYear();

    // Mobile menu toggle
    const hamburger = document.querySelector('.hamburger');
    const navLinks = document.querySelector('.nav-links');

    if (hamburger) {
        hamburger.addEventListener('click', () => {
            navLinks.classList.toggle('active');
            hamburger.classList.toggle('active');
        });
    }

    // Smooth scrolling for anchor links
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', function (e) {
            e.preventDefault();

            const targetId = this.getAttribute('href');
            if (targetId === '#') return;

            const targetElement = document.querySelector(targetId);
            if (targetElement) {
                window.scrollTo({
                    top: targetElement.offsetTop - 80,
                    behavior: 'smooth'
                });

                // Close mobile menu if open
                if (navLinks.classList.contains('active')) {
                    navLinks.classList.remove('active');
                    hamburger.classList.remove('active');
                }
            }
        });
    });

    // AI Terminal interaction
    const aiPromptInput = document.getElementById('ai-prompt');
    const sendPromptButton = document.getElementById('send-prompt');
    const aiResponseArea = document.getElementById('ai-response');
    const terminalOutput = document.getElementById('terminal-output');
    const executedCommand = document.getElementById('executed-command');

    // Sample responses for demo
    const sampleResponses = {
        "find large files": {
            response: `<p>I can help with that. Here's a command to find the largest files in the current directory:</p>
                      <pre><code>find . -type f -exec du -h {} \\; | sort -rh | head -n 10</code></pre>
                      <p>This will show the 10 largest files. Would you like me to explain how it works?</p>`,
            command: "find . -type f -exec du -h {} \\; | sort -rh | head -n 10"
        },
        "extract tar.gz": {
            response: `<p>To extract a tar.gz file, you can use this command:</p>
                      <pre><code>tar -xzf filename.tar.gz</code></pre>
                      <p>Replace 'filename.tar.gz' with your actual file name.</p>`,
            command: "tar -xzf filename.tar.gz"
        },
        "backup script": {
            response: `<p>Here's a simple backup script for you:</p>
                      <pre><code>#!/bin/bash

BACKUP_DIR="/path/to/backup"
SOURCE_DIR="/path/to/source"

mkdir -p $BACKUP_DIR
tar -czf $BACKUP_DIR/backup-$(date +%Y%m%d).tar.gz $SOURCE_DIR

echo "Backup completed!"</code></pre>
                      <p>Save this to a file (e.g., backup.sh), make it executable with <code>chmod +x backup.sh</code>, and run it.</p>`,
            command: "nano backup.sh"
        },
        "explain grep": {
            response: `<p>grep is a powerful search tool that finds patterns in text. Basic syntax:</p>
                      <pre><code>grep [options] pattern [file...]</code></pre>
                      <p>Common options:</p>
                      <ul>
                          <li><code>-i</code>: ignore case</li>
                          <li><code>-r</code>: recursive search</li>
                          <li><code>-v</code>: invert match</li>
                          <li><code>-n</code>: show line numbers</li>
                      </ul>
                      <p>Example: <code>grep -r "TODO" .</code> finds all TODOs in current directory</p>`,
            command: "grep -r \"TODO\" ."
        }
    };

    // Function to handle AI prompt submission
    function handlePromptSubmission() {
        if (!aiPromptInput || !aiPromptInput.value.trim()) return;

        const userQuery = aiPromptInput.value.trim();

        // Add user query to the response area
        const userQueryElement = document.createElement('div');
        userQueryElement.classList.add('user-query');
        userQueryElement.textContent = userQuery;
        aiResponseArea.appendChild(userQueryElement);

        // Find a matching response or use default
        let responseData = null;

        // Simple keyword matching
        for (const [key, data] of Object.entries(sampleResponses)) {
            if (userQuery.toLowerCase().includes(key)) {
                responseData = data;
                break;
            }
        }

        // Default response if no match
        if (!responseData) {
            responseData = {
                response: `<p>I'm not sure how to help with that specific query. Try asking about finding files, extracting archives, creating backup scripts, or using grep.</p>`,
                command: ""
            };
        }

        // Add AI response after a short delay
        setTimeout(() => {
            const aiResponseElement = document.createElement('div');
            aiResponseElement.classList.add('ai-response');
            aiResponseElement.innerHTML = responseData.response;
            aiResponseArea.appendChild(aiResponseElement);

            // Auto scroll to bottom
            aiResponseArea.scrollTop = aiResponseArea.scrollHeight;

            // Add command to terminal if there is one
            if (responseData.command) {
                setTimeout(() => {
                    // Add the command to the terminal
                    if (executedCommand) {
                        executedCommand.textContent = responseData.command;
                    }

                    // Add a new prompt line
                    setTimeout(() => {
                        const newLine = document.createElement('div');
                        newLine.classList.add('line');
                        newLine.innerHTML = `<span class="prompt">$</span><span class="command"></span>`;
                        terminalOutput.appendChild(newLine);

                        // Auto scroll terminal
                        terminalOutput.scrollTop = terminalOutput.scrollHeight;
                    }, 500);
                }, 1000);
            }
        }, 800);

        // Clear input
        aiPromptInput.value = '';
    }

    // Event listeners for AI prompt
    if (sendPromptButton) {
        sendPromptButton.addEventListener('click', handlePromptSubmission);
    }

    if (aiPromptInput) {
        aiPromptInput.addEventListener('keypress', function (e) {
            if (e.key === 'Enter') {
                handlePromptSubmission();
            }
        });
    }

    // Form submission
    const signupForm = document.getElementById('signup-form');
    const formMessage = document.getElementById('form-message');

    if (signupForm) {
        signupForm.addEventListener('submit', function (e) {
            e.preventDefault();

            const email = document.getElementById('email').value;

            // Simulate form submission
            formMessage.innerHTML = '<div class="success-message">Thanks for signing up! We\'ll be in touch soon.</div>';
            signupForm.reset();

            // In a real implementation, you would send this data to your server
            console.log('Email submitted:', email);
        });
    }

    // Add scroll animation for elements
    const observerOptions = {
        threshold: 0.1,
        rootMargin: '0px 0px -50px 0px'
    };

    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.classList.add('animate');
                observer.unobserve(entry.target);
            }
        });
    }, observerOptions);

    document.querySelectorAll('.feature-card, .demo-terminal, .signup-content').forEach(el => {
        el.classList.add('fade-in');
        observer.observe(el);
    });

    // Add this CSS dynamically for animations
    const style = document.createElement('style');
    style.textContent = `
        .fade-in {
            opacity: 0;
            transform: translateY(20px);
            transition: opacity 0.6s ease, transform 0.6s ease;
        }
        
        .fade-in.animate {
            opacity: 1;
            transform: translateY(0);
        }
        
        .nav-links.active {
            display: flex;
            flex-direction: column;
            position: absolute;
            top: 70px;
            left: 0;
            width: 100%;
            background-color: white;
            padding: 20px;
            box-shadow: 0 10px 20px rgba(0,0,0,0.1);
        }
        
        .hamburger.active span:nth-child(1) {
            transform: rotate(45deg) translate(5px, 5px);
        }
        
        .hamburger.active span:nth-child(2) {
            opacity: 0;
        }
        
        .hamburger.active span:nth-child(3) {
            transform: rotate(-45deg) translate(7px, -6px);
        }
    `;
    document.head.appendChild(style);

    // Interactive Terminal Demo
    function setupInteractiveDemo() {
        const demoTerminal = document.querySelector('.demo-terminal .terminal-body .terminal-left');
        const demoInput = document.createElement('div');
        demoInput.classList.add('terminal-input');
        demoInput.innerHTML = `
            <div class="line">
                <span class="prompt">$</span>
                <span class="command" contenteditable="true"></span>
            </div>
        `;

        if (demoTerminal) {
            demoTerminal.appendChild(demoInput);

            const commandSpan = demoInput.querySelector('.command');

            // Focus the input when clicking anywhere in the terminal
            demoTerminal.addEventListener('click', () => {
                commandSpan.focus();
            });

            // Handle command execution
            commandSpan.addEventListener('keydown', (e) => {
                if (e.key === 'Enter') {
                    e.preventDefault();

                    const command = commandSpan.textContent.trim();
                    if (!command) return;

                    // Create a new line with the executed command
                    const newCommandLine = document.createElement('div');
                    newCommandLine.classList.add('line');
                    newCommandLine.innerHTML = `
                        <span class="prompt">$</span>
                        <span class="command">${command}</span>
                    `;

                    // Insert before the input line
                    demoTerminal.insertBefore(newCommandLine, demoInput);

                    // Generate response based on command
                    const response = generateTerminalResponse(command);
                    if (response) {
                        const outputDiv = document.createElement('div');
                        outputDiv.classList.add('terminal-output');
                        outputDiv.textContent = response;
                        demoTerminal.insertBefore(outputDiv, demoInput);
                    }

                    // Clear the input
                    commandSpan.textContent = '';

                    // Scroll to bottom
                    demoTerminal.scrollTop = demoTerminal.scrollHeight;
                }
            });
        }
    }

    // Generate responses for demo commands
    function generateTerminalResponse(command) {
        const responses = {
            'ls': 'README.md  package.json  src/  node_modules/  .git/',
            'pwd': '/home/user/projects/terminal-ai',
            'echo hello': 'hello',
            'date': new Date().toString(),
            'help': 'Available commands: ls, pwd, echo, date, help, clear',
            'clear': null // Special case to clear the terminal
        };

        // Handle the clear command
        if (command === 'clear') {
            setTimeout(() => {
                const demoTerminal = document.querySelector('.demo-terminal .terminal-body .terminal-left');
                const inputLine = document.querySelector('.demo-terminal .terminal-input');

                // Remove all children except the input line
                while (demoTerminal.firstChild !== inputLine) {
                    demoTerminal.removeChild(demoTerminal.firstChild);
                }
            }, 100);
            return null;
        }

        // Check for exact matches
        if (responses[command]) {
            return responses[command];
        }

        // Check for commands with arguments
        if (command.startsWith('echo ')) {
            return command.substring(5);
        }

        // Default response for unknown commands
        return `Command not found: ${command}. Type 'help' to see available commands.`;
    }

    // Call the setup function when the page loads
    setupInteractiveDemo();

    // Add this function to create a typing animation effect
    function setupTypingAnimation() {
        const terminalCommands = document.querySelectorAll('.hero .terminal-body .terminal-left .line .command');

        // Skip the last empty command line
        const commandsToAnimate = Array.from(terminalCommands).slice(0, -1);

        let currentIndex = 0;

        function animateNextCommand() {
            if (currentIndex >= commandsToAnimate.length) {
                // Restart animation after a delay
                setTimeout(() => {
                    // Clear all commands
                    commandsToAnimate.forEach(cmd => {
                        cmd.textContent = '';
                    });

                    // Hide all outputs
                    document.querySelectorAll('.hero .terminal-body .terminal-left .terminal-output').forEach(output => {
                        output.style.display = 'none';
                    });

                    currentIndex = 0;
                    animateNextCommand();
                }, 5000);
                return;
            }

            const commandElement = commandsToAnimate[currentIndex];
            const originalText = commandElement.getAttribute('data-original') || commandElement.textContent;

            // Store original text if not already stored
            if (!commandElement.getAttribute('data-original')) {
                commandElement.setAttribute('data-original', originalText);
            }

            // Clear the command
            commandElement.textContent = '';

            // Type the command character by character
            let charIndex = 0;
            const typingInterval = setInterval(() => {
                if (charIndex < originalText.length) {
                    commandElement.textContent += originalText.charAt(charIndex);
                    charIndex++;
                } else {
                    clearInterval(typingInterval);

                    // Show the output after typing is complete
                    const outputElement = commandElement.closest('.line').nextElementSibling;
                    if (outputElement && outputElement.classList.contains('terminal-output')) {
                        outputElement.style.display = 'block';
                    }

                    // Move to the next command after a delay
                    setTimeout(() => {
                        currentIndex++;
                        animateNextCommand();
                    }, 1000);
                }
            }, 50 + Math.random() * 50); // Random typing speed for realism
        }

        // Start the animation
        animateNextCommand();
    }

    setupTypingAnimation();

    // Add theme toggle functionality
    function setupThemeToggle() {
        // Create the theme toggle button
        const themeToggle = document.createElement('div');
        themeToggle.classList.add('theme-toggle');
        themeToggle.innerHTML = `
            <i class="fas fa-moon"></i>
            <span class="toggle-slider"></span>
            <i class="fas fa-sun"></i>
        `;

        // Add it to the navigation
        const nav = document.querySelector('nav');
        if (nav) {
            nav.appendChild(themeToggle);
        }

        // Check for saved theme preference
        const savedTheme = localStorage.getItem('theme');
        if (savedTheme === 'light') {
            document.body.classList.add('light-theme');
            themeToggle.classList.add('active');
        }

        // Toggle theme on click
        themeToggle.addEventListener('click', () => {
            document.body.classList.toggle('light-theme');
            themeToggle.classList.toggle('active');

            // Save preference
            const currentTheme = document.body.classList.contains('light-theme') ? 'light' : 'dark';
            localStorage.setItem('theme', currentTheme);
        });
    }

    setupThemeToggle();

    // Add particle background effect to the hero section
    function setupParticleBackground() {
        // Create a container for particles
        const particlesContainer = document.createElement('div');
        particlesContainer.id = 'particles-js';

        // Add it to the hero section
        const heroSection = document.querySelector('.hero');
        if (heroSection) {
            heroSection.insertBefore(particlesContainer, heroSection.firstChild);

            // Load particles.js from CDN if not already loaded
            if (!window.particlesJS) {
                const script = document.createElement('script');
                script.src = 'https://cdn.jsdelivr.net/particles.js/2.0.0/particles.min.js';
                script.onload = initParticles;
                document.head.appendChild(script);
            } else {
                initParticles();
            }
        }
    }

    function initParticles() {
        particlesJS('particles-js', {
            particles: {
                number: { value: 80, density: { enable: true, value_area: 800 } },
                color: { value: '#00b0ff' },
                shape: { type: 'circle' },
                opacity: { value: 0.5, random: false },
                size: { value: 3, random: true },
                line_linked: {
                    enable: true,
                    distance: 150,
                    color: '#00b0ff',
                    opacity: 0.4,
                    width: 1
                },
                move: {
                    enable: true,
                    speed: 2,
                    direction: 'none',
                    random: false,
                    straight: false,
                    out_mode: 'out',
                    bounce: false
                }
            },
            interactivity: {
                detect_on: 'canvas',
                events: {
                    onhover: { enable: true, mode: 'grab' },
                    onclick: { enable: true, mode: 'push' },
                    resize: true
                },
                modes: {
                    grab: { distance: 140, line_linked: { opacity: 1 } },
                    push: { particles_nb: 4 }
                }
            },
            retina_detect: true
        });
    }

    setupParticleBackground();

    // Add animated counters for GitHub stats
    function setupAnimatedCounters() {
        const stats = document.querySelectorAll('.github-stats .stat span');

        // Set up Intersection Observer to trigger animation when stats are visible
        const observer = new IntersectionObserver((entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    animateCounter(entry.target);
                    observer.unobserve(entry.target);
                }
            });
        }, { threshold: 0.5 });

        stats.forEach(stat => {
            // Store the target value
            const targetValue = parseInt(stat.textContent.match(/\d+/)[0]);
            stat.setAttribute('data-target', targetValue);
            stat.textContent = '0';

            // Observe the stat
            observer.observe(stat);
        });
    }

    function animateCounter(element) {
        const target = parseInt(element.getAttribute('data-target'));
        let count = 0;
        const duration = 2000; // 2 seconds
        const frameRate = 60;
        const totalFrames = duration / (1000 / frameRate);
        const increment = target / totalFrames;

        element.classList.add('counter-animation');

        const counter = setInterval(() => {
            count += increment;

            // Update the element text
            if (count >= target) {
                element.textContent = target;
                clearInterval(counter);
            } else {
                element.textContent = Math.floor(count);
            }
        }, 1000 / frameRate);
    }

    setupAnimatedCounters();
}); 