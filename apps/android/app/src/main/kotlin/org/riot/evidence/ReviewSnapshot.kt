package org.riot.evidence

data class ReviewSnapshot(
    val headline: String,
    val description: String,
    val aiAssisted: Boolean,
) {
    fun matches(headline: String, description: String, aiAssisted: Boolean): Boolean =
        this.headline == headline.trim() &&
            this.description == description.trim() &&
            this.aiAssisted == aiAssisted

    companion object {
        fun capture(headline: String, description: String, aiAssisted: Boolean) = ReviewSnapshot(
            headline.trim(),
            description.trim(),
            aiAssisted,
        )
    }
}
