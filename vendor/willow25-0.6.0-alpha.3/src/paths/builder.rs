use core::fmt;

use ufotofu::codec_prelude::*;

use willow_data_model::prelude as wdm;

use crate::prelude::*;

wrapper! {
    /// A helper struct for creating a [`Path`] with exactly one memory allocation. Requires total length and component count to be known in advance.
    ///
    /// Enforces that each [`Component`] has a length of at most 4096 ([`MCL`]), that each [`Path`] has at most 4096 ([`MCC`]) [`Component`]s, and that the total size in bytes of all [`Component`]s is at most 4096 ([`MPL`]).
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// builder.append_component(component!("hi"));
    /// builder.append_slice(b"ho")?;
    /// assert_eq!(builder.build(), path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    PathBuilder; wdm::PathBuilder<MCL, MCC, MPL>
}

impl fmt::Debug for PathBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PathBuilder {
    /// Creates a builder for a [`Path`] of known total length and component count.
    /// The component data must be filled in before building.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(total_length + component_count)`, performs a single allocation of `O(total_length + component_count)` bytes.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// builder.append_component(component!("hi"));
    /// builder.append_slice(b"ho")?;
    /// assert_eq!(builder.build(), path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn new(
        total_length: usize,
        component_count: usize,
    ) -> Result<Self, PathFromComponentsError> {
        Ok(Self(wdm::PathBuilder::new(total_length, component_count)?))
    }

    /// Creates a builder for a [`Path`] of known total length and component count, efficiently prefilled with the first `prefix_component_count` [`Component`]s of a given `reference` [`Path`]. Panics if there are not enough [`Component`]s in the `reference`.
    ///
    /// The missing component data must be filled in before building.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(target_total_length + target_component_count)`, performs a single allocation of `O(total_length + component_count)` bytes.
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let p = path!("/hi/he");
    /// let mut builder = PathBuilder::new_from_prefix(4, 2, &p, 1)?;
    /// builder.append_component(component!("ho"));
    /// assert_eq!(builder.build(), path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn new_from_prefix(
        target_total_length: usize,
        target_component_count: usize,
        reference: &Path,
        prefix_component_count: usize,
    ) -> Result<Self, PathFromComponentsError> {
        Ok(Self(wdm::PathBuilder::new_from_prefix(
            target_total_length,
            target_component_count,
            reference.into(),
            prefix_component_count,
        )?))
    }

    /// Appends the data for the next [`Component`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(component_length)` time. Performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// builder.append_component(component!("hi"));
    /// builder.append_component(component!("ho"));
    /// assert_eq!(builder.build(), path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_component(&mut self, component: &Component) {
        self.0.append_component(component.into());
    }

    /// Appends the data for the next [`Component`], from a slice of bytes.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(component_length)` time. Performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// builder.append_slice(b"hi")?;
    /// builder.append_slice(b"ho")?;
    /// assert_eq!(builder.build(), path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn append_slice(&mut self, component: &[u8]) -> Result<(), InvalidComponentError> {
        self.0.append_slice(component)
    }

    /// Appends data for a component of known length by reading data from a [`BulkProducer`] of bytes. Panics if `component_length > MCL`.
    ///
    /// #### Complexity
    ///
    /// Runs in `O(component_length)` time (assuming the producer takes `O(1)` time per byte). Performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// use ufotofu::{codec_prelude::*, producer::clone_from_slice};
    ///
    /// # pollster::block_on(async {
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// builder.append_component_from_bulk_producer(2, &mut clone_from_slice(b"hi")).await.unwrap();
    /// builder.append_component_from_bulk_producer(2, &mut clone_from_slice(b"ho")).await.unwrap();
    /// assert_eq!(builder.build(), Path::from_slices(&[b"hi", b"ho"])?);
    /// # Ok::<(), PathError>(())
    /// # });
    /// ```
    pub async fn append_component_from_bulk_producer<P>(
        &mut self,
        component_length: usize,
        p: &mut P,
    ) -> Result<(), DecodeError<P::Final, P::Error, Blame>>
    where
        P: BulkProducer<Item = u8> + ?Sized,
    {
        self.0
            .append_component_from_bulk_producer(component_length, p)
            .await
    }

    /// Turns this builder into an immutable [`Path`].
    ///
    /// Panics if the number of [`Component`]s or the total length does not match what was claimed in [`PathBuilder::new`].
    ///
    /// #### Complexity
    ///
    /// Runs in `O(1)` time. Performs no allocations.
    ///
    /// #### Examples
    ///
    /// ```
    /// use willow25::prelude::*;
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// builder.append_component(component!("hi"));
    /// builder.append_slice(b"ho")?;
    /// assert_eq!(builder.build(), path!("/hi/ho"));
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn build(self) -> Path {
        self.0.build().into()
    }

    /// Returns the total length of all components added to the builder so far.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// assert_eq!(builder.total_length(), 0);
    ///
    /// builder.append_component(Component::new(b"hi")?);
    /// assert_eq!(builder.total_length(), 2);
    ///
    /// builder.append_slice(b"ho")?;
    /// assert_eq!(builder.total_length(), 4);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn total_length(&self) -> usize {
        self.0.total_length()
    }

    /// Returns the number of components added to the builder so far.
    ///
    /// ```
    /// use willow25::prelude::*;
    ///
    /// let mut builder = PathBuilder::new(4, 2)?;
    /// assert_eq!(builder.component_count(), 0);
    ///
    /// builder.append_component(Component::new(b"hi")?);
    /// assert_eq!(builder.component_count(), 1);
    ///
    /// builder.append_slice(b"ho")?;
    /// assert_eq!(builder.component_count(), 2);
    /// # Ok::<(), PathError>(())
    /// ```
    pub fn component_count(&self) -> usize {
        self.0.component_count()
    }
}
